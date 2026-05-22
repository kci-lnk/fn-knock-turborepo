import { isIP } from "node:net";
import { normalizeCidrLines } from "../../../../packages/admin-shared/src/utils/cidr";
import { cidrService } from "./cidr";
import {
  goBackend,
  type CommonLocationExemptionsRuntime,
} from "./go-backend";
import {
  type IpLocationResult,
  ipLocationService,
} from "./ip-location";
import {
  recentAuthIPsManager,
  type RecentAuthIPEntry,
} from "./recent-auth-ips";
import { configManager, redis } from "./redis";
import { isWhitelistExemptIp, normalizeIp } from "./ip-normalize";
import { isIpMatchedByCIDR } from "./whitelist-target";

export type CommonAuthLocationConfidence = "high" | "medium" | "low";
export type CommonAuthLocationCIDRSource = "region" | "sample" | "mixed";

export interface CommonAuthLocationRuntimeLocation {
  key: string;
  label: string;
  country: string;
  province: string;
  city: string;
  isp: string;
  ip_count: number;
  seen_count: number;
  ips: string[];
  first_seen_at: number;
  last_seen_at: number;
  score: number;
  confidence: CommonAuthLocationConfidence;
  cidrs: string[];
  cidr_source: CommonAuthLocationCIDRSource;
  cidr_error?: string;
}

export interface CommonAuthLocationsRuntimeState {
  enabled: boolean;
  cidrs: string[];
  locations: CommonAuthLocationRuntimeLocation[];
  sample_count: number;
  resolved_sample_count: number;
  pending_ip_count: number;
  updated_at: string | null;
}

type ResolvedSample = RecentAuthIPEntry & {
  location: IpLocationResult;
};

type LocationGroup = {
  key: string;
  country: string;
  province: string;
  city: string;
  isp: string;
  samples: ResolvedSample[];
};

const RUNTIME_KEY = "fn_knock:common_auth_locations:runtime";
const DEFAULT_REBUILD_DEBOUNCE_MS = Math.max(
  1000,
  Number.parseInt(
    process.env.COMMON_AUTH_LOCATIONS_REBUILD_DEBOUNCE_MS || "5000",
    10,
  ) || 5000,
);
const LOCATION_REFRESH_RETRY_MS = Math.max(
  5000,
  Number.parseInt(
    process.env.COMMON_AUTH_LOCATIONS_LOCATION_RETRY_MS || "30000",
    10,
  ) || 30000,
);
const MAX_RECENT_IPS = Math.max(
  10,
  Number.parseInt(process.env.COMMON_AUTH_LOCATIONS_MAX_IPS || "1000", 10) ||
    1000,
);
const MAX_LOCATIONS = Math.max(
  1,
  Number.parseInt(
    process.env.COMMON_AUTH_LOCATIONS_MAX_LOCATIONS || "5",
    10,
  ) || 5,
);
const MAX_CIDRS = Math.max(
  1,
  Number.parseInt(process.env.COMMON_AUTH_LOCATIONS_MAX_CIDRS || "1000", 10) ||
    1000,
);
const parsedMaxRegionCidrsPerLocation = Number.parseInt(
  process.env.COMMON_AUTH_LOCATIONS_MAX_REGION_CIDRS_PER_LOCATION || "128",
  10,
);
const MAX_REGION_CIDRS_PER_LOCATION = Number.isFinite(
  parsedMaxRegionCidrsPerLocation,
)
  ? Math.max(0, parsedMaxRegionCidrsPerLocation)
  : 128;
const RECENT_WINDOW_SECONDS = 7 * 24 * 3600;
const KNOWN_COUNTRY_CHINA = "中国";

let scheduledRebuildTimer: ReturnType<typeof setTimeout> | null = null;
let scheduledRebuildReason = "scheduled";
let rebuildInFlight: Promise<CommonAuthLocationsRuntimeState> | null = null;
let rebuildRerunRequested = false;
let runtimeEndpointUnavailableLogged = false;

const emptyRuntimeState = (): CommonAuthLocationsRuntimeState => ({
  enabled: false,
  cidrs: [],
  locations: [],
  sample_count: 0,
  resolved_sample_count: 0,
  pending_ip_count: 0,
  updated_at: null,
});

const normalizeString = (value: unknown): string =>
  String(value ?? "").trim();

const normalizeRuntimeLocation = (
  value: Partial<CommonAuthLocationRuntimeLocation> | null | undefined,
): CommonAuthLocationRuntimeLocation | null => {
  const raw = value ?? {};
  const key = normalizeString(raw.key);
  if (!key) return null;

  const confidence =
    raw.confidence === "high" || raw.confidence === "medium"
      ? raw.confidence
      : "low";
  const cidrSource =
    raw.cidr_source === "region" || raw.cidr_source === "sample"
      ? raw.cidr_source
      : "mixed";

  return {
    key,
    label: normalizeString(raw.label) || key,
    country: normalizeString(raw.country),
    province: normalizeString(raw.province),
    city: normalizeString(raw.city),
    isp: normalizeString(raw.isp),
    ip_count: Math.max(0, Math.floor(Number(raw.ip_count) || 0)),
    seen_count: Math.max(0, Math.floor(Number(raw.seen_count) || 0)),
    ips: Array.isArray(raw.ips)
      ? [...new Set(raw.ips.map((ip) => normalizeIp(String(ip))).filter(Boolean))]
      : [],
    first_seen_at: Math.max(0, Math.floor(Number(raw.first_seen_at) || 0)),
    last_seen_at: Math.max(0, Math.floor(Number(raw.last_seen_at) || 0)),
    score: Math.max(0, Number(raw.score) || 0),
    confidence,
    cidrs: normalizeCidrLines(
      Array.isArray(raw.cidrs) ? raw.cidrs.map((cidr) => String(cidr)) : [],
    ),
    cidr_source: cidrSource,
    ...(normalizeString(raw.cidr_error)
      ? { cidr_error: normalizeString(raw.cidr_error) }
      : {}),
  };
};

const normalizeRuntimeState = (
  value?: Partial<CommonAuthLocationsRuntimeState> | null,
): CommonAuthLocationsRuntimeState => {
  const raw = value ?? {};
  const locations = Array.isArray(raw.locations)
    ? raw.locations
        .map((item) => normalizeRuntimeLocation(item))
        .filter(
          (item): item is CommonAuthLocationRuntimeLocation => item !== null,
        )
    : [];
  const cidrs = normalizeCidrLines(
    Array.isArray(raw.cidrs) ? raw.cidrs.map((cidr) => String(cidr)) : [],
  );

  return {
    enabled: raw.enabled === true && cidrs.length > 0,
    cidrs,
    locations,
    sample_count: Math.max(0, Math.floor(Number(raw.sample_count) || 0)),
    resolved_sample_count: Math.max(
      0,
      Math.floor(Number(raw.resolved_sample_count) || 0),
    ),
    pending_ip_count: Math.max(0, Math.floor(Number(raw.pending_ip_count) || 0)),
    updated_at: normalizeString(raw.updated_at) || null,
  };
};

const getLocationKey = (location: IpLocationResult): string => {
  const country = normalizeString(location.country);
  const province = normalizeString(location.province);
  const city = normalizeString(location.city);
  if (!country && !province && !city) return "";
  return [country, province, city].filter(Boolean).join("|");
};

const getLocationLabel = (group: LocationGroup): string => {
  if (group.country === KNOWN_COUNTRY_CHINA) {
    return [group.province, group.city, group.isp].filter(Boolean).join(" / ");
  }
  return [group.country, group.province, group.city, group.isp]
    .filter(Boolean)
    .join(" / ");
};

const scoreGroup = (
  group: LocationGroup,
  nowSeconds: number,
): {
  firstSeenAt: number;
  lastSeenAt: number;
  seenCount: number;
  score: number;
  confidence: CommonAuthLocationConfidence;
} => {
  const firstSeenAt = Math.min(...group.samples.map((item) => item.firstSeenAt));
  const lastSeenAt = Math.max(...group.samples.map((item) => item.lastSeenAt));
  const seenCount = group.samples.reduce(
    (total, item) => total + Math.max(1, item.seenCount),
    0,
  );
  const ageSeconds = Math.max(0, nowSeconds - lastSeenAt);
  const recent = ageSeconds <= RECENT_WINDOW_SECONDS;
  const recencyScore = Math.max(0, 30 - ageSeconds / 86400);
  const score =
    group.samples.length * 100 + Math.min(seenCount, 50) * 5 + recencyScore;
  const confidence: CommonAuthLocationConfidence =
    (recent && (group.samples.length >= 3 || seenCount >= 10)) ||
    (group.samples.length >= 2 && seenCount >= 8)
      ? "high"
      : group.samples.length >= 2 || seenCount >= 5
        ? "medium"
        : "low";

  return {
    firstSeenAt,
    lastSeenAt,
    seenCount,
    score,
    confidence,
  };
};

const toExactSampleCIDR = (ip: string): string => {
  const family = isIP(ip);
  if (family === 4) return `${ip}/32`;
  if (family === 6) return `${ip}/128`;
  return "";
};

const deriveIPv4BucketCIDRs = (ips: string[]): string[] => {
  const buckets = new Map<string, number>();
  for (const ip of ips) {
    if (isIP(ip) !== 4) continue;
    const parts = ip.split(".");
    if (parts.length !== 4) continue;
    const bucket = `${parts[0]}.${parts[1]}.${parts[2]}.0/24`;
    buckets.set(bucket, (buckets.get(bucket) ?? 0) + 1);
  }

  return [...buckets.entries()]
    .filter(([, count]) => count >= 2)
    .map(([cidr]) => cidr);
};

const deriveSampleCIDRs = (group: LocationGroup): string[] => {
  const ips = group.samples.map((sample) => sample.ip);
  return normalizeCidrLines([
    ...deriveIPv4BucketCIDRs(ips),
    ...ips.map(toExactSampleCIDR).filter(Boolean),
  ]);
};

const resolveRegionCIDRs = async (
  group: LocationGroup,
): Promise<{ cidrs: string[]; error?: string }> => {
  if (group.country !== KNOWN_COUNTRY_CHINA || !group.province || !group.city) {
    return { cidrs: [] };
  }

  try {
    const lookup = await cidrService.getCidrs({
      province: group.province,
      city: group.city,
    });
    return {
      cidrs: normalizeCidrLines([
        ...lookup.cidrGroups.ipv4,
        ...lookup.cidrGroups.ipv6,
      ]),
    };
  } catch (error) {
    return {
      cidrs: [],
      error: error instanceof Error ? error.message : "CIDR 查询失败",
    };
  }
};

const buildLocationGroups = (samples: ResolvedSample[]): LocationGroup[] => {
  const groups = new Map<string, LocationGroup>();

  for (const sample of samples) {
    const key = getLocationKey(sample.location);
    if (!key) continue;

    const existing = groups.get(key);
    if (existing) {
      const sampleIsp = normalizeString(sample.location.isp);
      if (existing.isp && existing.isp !== sampleIsp) {
        existing.isp = "";
      }
      existing.samples.push(sample);
      continue;
    }

    groups.set(key, {
      key,
      country: normalizeString(sample.location.country),
      province: normalizeString(sample.location.province),
      city: normalizeString(sample.location.city),
      isp: normalizeString(sample.location.isp),
      samples: [sample],
    });
  }

  return [...groups.values()];
};

const getResolvedSamples = async (
  entries: RecentAuthIPEntry[],
): Promise<{ samples: ResolvedSample[]; pendingIps: string[] }> => {
  const samples: ResolvedSample[] = [];
  const pendingIps: string[] = [];

  for (const entry of entries) {
    const cached = await ipLocationService.getCachedResult(entry.ip);
    if (!cached) {
      pendingIps.push(entry.ip);
      continue;
    }

    samples.push({
      ...entry,
      location: cached,
    });
  }

  if (pendingIps.length > 0) {
    await ipLocationService.ensureEnqueuedBatch(pendingIps);
  }

  return { samples, pendingIps };
};

const persistRuntimeState = async (
  state: CommonAuthLocationsRuntimeState,
): Promise<CommonAuthLocationsRuntimeState> => {
  const normalized = normalizeRuntimeState(state);
  await redis.set(RUNTIME_KEY, JSON.stringify(normalized));
  return normalized;
};

export const getCommonAuthLocationsRuntimeState =
  async (): Promise<CommonAuthLocationsRuntimeState> => {
    const raw = await redis.get(RUNTIME_KEY);
    if (!raw) return emptyRuntimeState();

    try {
      return normalizeRuntimeState(JSON.parse(raw));
    } catch {
      return emptyRuntimeState();
    }
  };

const toGatewayPayload = async (
  runtime: CommonAuthLocationsRuntimeState,
): Promise<CommonLocationExemptionsRuntime> => {
  const wafConfig = await configManager.getWAFConfig();
  const enabled =
    wafConfig.enabled &&
    wafConfig.common_location_exempt_enabled === true &&
    runtime.enabled &&
    runtime.cidrs.length > 0;

  return {
    enabled,
    waf_enabled: enabled,
    cidrs: enabled ? runtime.cidrs : [],
    updated_at: runtime.updated_at,
  };
};

export const syncCommonAuthLocationExemptionsToGateway = async (
  runtime?: CommonAuthLocationsRuntimeState | null,
): Promise<CommonAuthLocationsRuntimeState> => {
  const nextRuntime =
    runtime ?? (await getCommonAuthLocationsRuntimeState());
  const response = await goBackend.setCommonLocationExemptions(
    await toGatewayPayload(nextRuntime),
  );

  if (!response.success) {
    if (response.code === 404 || response.code === 501) {
      if (!runtimeEndpointUnavailableLogged) {
        runtimeEndpointUnavailableLogged = true;
        console.warn(
          "[common-auth-locations] Go runtime endpoint /api/runtime/common-location-exemptions is unavailable; skipping WAF exemption sync until the Go backend supports it.",
        );
      }
      return nextRuntime;
    }

    throw new Error(response.message || "同步常用地豁免配置到网关失败");
  }

  runtimeEndpointUnavailableLogged = false;
  return nextRuntime;
};

const rebuildRuntimeOnce = async (): Promise<CommonAuthLocationsRuntimeState> => {
  const entries = (await recentAuthIPsManager.listActiveWithScores(MAX_RECENT_IPS))
    .map((entry) => ({
      ...entry,
      ip: normalizeIp(entry.ip),
    }))
    .filter((entry) => entry.ip && !isWhitelistExemptIp(entry.ip));
  const { samples, pendingIps } = await getResolvedSamples(entries);
  const nowSeconds = Math.floor(Date.now() / 1000);
  const groups = buildLocationGroups(samples)
    .map((group) => ({
      group,
      ...scoreGroup(group, nowSeconds),
    }))
    .filter((item) => item.confidence !== "low")
    .sort((left, right) => {
      const scoreDelta = right.score - left.score;
      if (scoreDelta !== 0) return scoreDelta;
      return right.lastSeenAt - left.lastSeenAt;
    })
    .slice(0, MAX_LOCATIONS);

  const locations: CommonAuthLocationRuntimeLocation[] = [];
  const allCidrs: string[] = [];
  const seenCidrs = new Set<string>();

  for (const item of groups) {
    if (allCidrs.length >= MAX_CIDRS) break;

    const regionResult = await resolveRegionCIDRs(item.group);
    const regionCidrs = regionResult.cidrs.slice(
      0,
      MAX_REGION_CIDRS_PER_LOCATION,
    );
    const sampleCidrs = deriveSampleCIDRs(item.group);
    const sampleCidrSet = new Set(sampleCidrs);
    const regionCidrSet = new Set(regionCidrs);
    const cidrs = normalizeCidrLines([...sampleCidrs, ...regionCidrs]);
    const selectedCidrs: string[] = [];
    let selectedSampleCidr = false;
    let selectedRegionCidr = false;
    for (const cidr of cidrs) {
      if (seenCidrs.has(cidr)) continue;
      selectedCidrs.push(cidr);
      seenCidrs.add(cidr);
      selectedSampleCidr ||= sampleCidrSet.has(cidr);
      selectedRegionCidr ||= regionCidrSet.has(cidr);
      if (allCidrs.length + selectedCidrs.length >= MAX_CIDRS) break;
    }
    allCidrs.push(...selectedCidrs);
    const cidrSource: CommonAuthLocationCIDRSource =
      selectedSampleCidr && selectedRegionCidr
        ? "mixed"
        : selectedRegionCidr
          ? "region"
          : "sample";

    locations.push({
      key: item.group.key,
      label: getLocationLabel(item.group),
      country: item.group.country,
      province: item.group.province,
      city: item.group.city,
      isp: item.group.isp,
      ip_count: item.group.samples.length,
      seen_count: item.seenCount,
      ips: item.group.samples.map((sample) => sample.ip).sort(),
      first_seen_at: item.firstSeenAt,
      last_seen_at: item.lastSeenAt,
      score: Number(item.score.toFixed(2)),
      confidence: item.confidence,
      cidrs: selectedCidrs,
      cidr_source: cidrSource,
      ...(regionResult.error ? { cidr_error: regionResult.error } : {}),
    });
  }

  const state = await persistRuntimeState({
    enabled: allCidrs.length > 0,
    cidrs: normalizeCidrLines(allCidrs),
    locations,
    sample_count: entries.length,
    resolved_sample_count: samples.length,
    pending_ip_count: pendingIps.length,
    updated_at: new Date().toISOString(),
  });

  await syncCommonAuthLocationExemptionsToGateway(state);

  if (pendingIps.length > 0) {
    scheduleCommonAuthLocationsRebuild({
      reason: "ip-location-refresh",
      delayMs: LOCATION_REFRESH_RETRY_MS,
    });
  }

  return state;
};

export const rebuildCommonAuthLocationsRuntimeState =
  async (): Promise<CommonAuthLocationsRuntimeState> => {
    if (rebuildInFlight) {
      rebuildRerunRequested = true;
      return rebuildInFlight;
    }

    rebuildInFlight = (async () => {
      let lastRuntime = await rebuildRuntimeOnce();

      while (rebuildRerunRequested) {
        rebuildRerunRequested = false;
        lastRuntime = await rebuildRuntimeOnce();
      }

      return lastRuntime;
    })();

    try {
      return await rebuildInFlight;
    } finally {
      rebuildInFlight = null;
    }
  };

export const scheduleCommonAuthLocationsRebuild = ({
  reason = "scheduled",
  delayMs = DEFAULT_REBUILD_DEBOUNCE_MS,
}: {
  reason?: string;
  delayMs?: number;
} = {}): void => {
  scheduledRebuildReason = reason;

  if (scheduledRebuildTimer) {
    clearTimeout(scheduledRebuildTimer);
  }

  scheduledRebuildTimer = setTimeout(
    () => {
      const nextReason = scheduledRebuildReason;
      scheduledRebuildTimer = null;
      scheduledRebuildReason = "scheduled";
      void rebuildCommonAuthLocationsRuntimeState().catch((error) => {
        console.error(
          `[common-auth-locations] failed to rebuild runtime (${nextReason}):`,
          error,
        );
      });
    },
    Math.max(0, Math.floor(delayMs)),
  );
  scheduledRebuildTimer.unref?.();
};

export const isCommonAuthLocationExemptIP = async (
  ip: string,
): Promise<boolean> => {
  const normalizedIp = normalizeIp(ip);
  if (!normalizedIp || isWhitelistExemptIp(normalizedIp)) {
    return false;
  }

  const runtime = await getCommonAuthLocationsRuntimeState();
  if (!runtime.enabled || runtime.cidrs.length === 0) {
    return false;
  }

  return runtime.cidrs.some((cidr) => isIpMatchedByCIDR(normalizedIp, cidr));
};
