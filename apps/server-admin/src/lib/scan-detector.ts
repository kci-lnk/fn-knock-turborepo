import { isAnySubdomainRoutingMode } from "./reverse-proxy-submode";
import {
  configManager,
  redis,
  type AppConfig,
  type HostMapping,
  type ProxyMapping,
} from "./redis";
import { ipLocationRefs, ipLocationService } from "./ip-location";
import { emitScannerBlockedEvent } from "./system-events/helpers";
import { isCommonAuthLocationExemptIP } from "./common-auth-locations";
import { cidrService } from "./cidr";
import {
  isScannerCidrExemptIp,
  normalizeScannerCidrExemptions,
  validateScannerCidrExemptions,
} from "./scanner-cidr";

type ScanHit = {
  path: string;
  createdAt: number;
};

type BlacklistRecord = {
  ip: string;
  blockedAt: number;
  windowMinutes: number;
  threshold: number;
  hits: ScanHit[];
  ipLocation?: string;
};

type ScannerSettings = {
  enabled: boolean;
  windowMinutes: number;
  threshold: number;
  windowSeconds: number;
  blacklistTtlSeconds: number;
  commonLocationExemptEnabled: boolean;
  cidrExemptions: string[];
  cidrExemptionRegions: ScannerCidrExemptionSelection[];
  cidrExemptionRegionCidrs: string[];
  cidrExemptionCidrs: string[];
};

type ScannerCidrExemptionSelection = {
  province: string;
  city: string | null;
  label: string;
  value: string;
  query_city: string | null;
  is_province_wide: boolean;
  is_municipality: boolean;
};

type ScannerCidrExemptionRegionInput = {
  province: string;
  query_city?: string | null;
};

const parseIntSafe = (value: string | undefined, fallback: number) => {
  const v = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(v)) return fallback;
  return v;
};

const normalizeString = (value: unknown): string => String(value ?? "").trim();

const scannerCidrRegionKey = (
  province: string,
  queryCity?: string | null,
): string => `${province.trim()}::${String(queryCity ?? "").trim()}`;

const scannerCidrRegionKeysEqual = (
  left: Array<{ province: string; query_city?: string | null }>,
  right: Array<{ province: string; query_city?: string | null }>,
): boolean => {
  if (left.length !== right.length) return false;
  return left.every(
    (item, index) =>
      scannerCidrRegionKey(item.province, item.query_city) ===
      scannerCidrRegionKey(right[index]?.province ?? "", right[index]?.query_city),
  );
};

const normalizeScannerCidrExemptionSelection = (
  value: unknown,
): ScannerCidrExemptionSelection | null => {
  if (!value || typeof value !== "object") return null;
  const raw = value as Partial<ScannerCidrExemptionSelection>;
  const province = normalizeString(raw.province);
  const label = normalizeString(raw.label);
  const valueLabel = normalizeString(raw.value);
  if (!province || !label || !valueLabel) return null;

  return {
    province,
    city: normalizeString(raw.city) || null,
    label,
    value: valueLabel,
    query_city: normalizeString(raw.query_city) || null,
    is_province_wide: raw.is_province_wide === true,
    is_municipality: raw.is_municipality === true,
  };
};

const normalizeScannerCidrExemptionRegions = (
  value: unknown,
): ScannerCidrExemptionSelection[] => {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => normalizeScannerCidrExemptionSelection(item))
    .filter((item): item is ScannerCidrExemptionSelection => item !== null);
};

const dedupeScannerCidrExemptionRegionInputs = (
  value: unknown,
): ScannerCidrExemptionRegionInput[] => {
  const items = Array.isArray(value) ? value : [];
  const result: ScannerCidrExemptionRegionInput[] = [];
  const seen = new Set<string>();

  for (const item of items) {
    if (!item || typeof item !== "object") continue;
    const raw = item as Partial<ScannerCidrExemptionRegionInput>;
    const province = normalizeString(raw.province);
    const queryCity = normalizeString(raw.query_city) || null;
    if (!province) continue;

    const key = scannerCidrRegionKey(province, queryCity);
    if (seen.has(key)) continue;
    seen.add(key);
    result.push({ province, query_city: queryCity });
  }

  return result;
};

const toStoredScannerCidrExemptionSelection = (
  value: Awaited<ReturnType<typeof cidrService.getCidrs>>["selection"],
): ScannerCidrExemptionSelection => ({
  province: value.province,
  city: value.city,
  label: value.label,
  value: value.value,
  query_city: value.queryCity,
  is_province_wide: value.isProvinceWide,
  is_municipality: value.isMunicipality,
});

const SUBSONIC_REST_ENDPOINTS = new Set([
  "addchatmessage",
  "changeemail",
  "changepassword",
  "createbookmark",
  "createinternetradiostation",
  "createplaylist",
  "createpodcastchannel",
  "createshare",
  "createuser",
  "deletebookmark",
  "deleteinternetradiostation",
  "deleteplaylist",
  "deletepodcastchannel",
  "deletepodcastepisode",
  "deleteshare",
  "deleteuser",
  "download",
  "downloadpodcastepisode",
  "getalbum",
  "getalbuminfo",
  "getalbuminfo2",
  "getalbumlist",
  "getalbumlist2",
  "getartist",
  "getartistinfo",
  "getartistinfo2",
  "getartists",
  "getavatar",
  "getbookmarks",
  "getchatmessages",
  "getcoverart",
  "getgenres",
  "getindexes",
  "getinternetradiostations",
  "getlicense",
  "getlyrics",
  "getlyricsbysongid",
  "getmusicdirectory",
  "getmusicfolders",
  "getnewestpodcasts",
  "getnowplaying",
  "getplaylists",
  "getplaylist",
  "getplayqueue",
  "getpodcasts",
  "getrandomsongs",
  "getshares",
  "getsimilarsongs",
  "getsimilarsongs2",
  "getsong",
  "getsongsbygenre",
  "getstarred",
  "getstarred2",
  "gettopsongs",
  "getuser",
  "getusers",
  "getvideoinfo",
  "getvideos",
  "hls",
  "jukeboxcontrol",
  "ping",
  "refreshpodcasts",
  "saveplayqueue",
  "scrobble",
  "search2",
  "search3",
  "setrating",
  "star",
  "stream",
  "unstar",
  "updateinternetradiostation",
  "updateplaylist",
  "updateshare",
  "updateuser",
]);

class ScanDetector {
  private readonly suspiciousPrefix = "fn_knock:scanner:suspicious:";
  private readonly blacklistIndexKey = "fn_knock:scanner:blacklist:index";
  private readonly blacklistDataPrefix = "fn_knock:scanner:blacklist:data:";
  private readonly settingsKey = "fn_knock:scanner:settings";
  private readonly baseWindowSeconds = 5 * 60;

  private suspiciousKey(ip: string) {
    return `${this.suspiciousPrefix}${ip}`;
  }

  private blacklistDataKey(ip: string) {
    return `${this.blacklistDataPrefix}${ip}`;
  }

  private normalizeIp(ip: string) {
    return ip.trim();
  }

  private normalizeHost(value: string) {
    const normalized = value.trim().toLowerCase();
    if (!normalized) return "";

    try {
      return new URL(`https://${normalized}`).hostname.toLowerCase();
    } catch {
      return normalized
        .replace(/^[a-z]+:\/\//i, "")
        .replace(/\/.*$/, "")
        .replace(/:\d+$/, "")
        .replace(/\.+$/, "");
    }
  }

  private resolveForwardedHost(request: Request) {
    const forwarded = request.headers
      .get("x-forwarded-host")
      ?.split(",")[0]
      ?.trim();
    if (forwarded) return this.normalizeHost(forwarded);

    try {
      return this.normalizeHost(new URL(request.url).host);
    } catch {
      return this.normalizeHost(request.headers.get("host") || "");
    }
  }

  private resolveForwardedPath(request: Request) {
    const forwarded = request.headers
      .get("x-forwarded-path")
      ?.split(",")[0]
      ?.trim();
    if (forwarded) return forwarded;

    try {
      const url = new URL(request.url);
      return `${url.pathname}${url.search}`;
    } catch {
      return "/";
    }
  }

  private isLocalAddress(ip: string) {
    const normalized = this.normalizeIp(ip).toLowerCase();
    if (!normalized) return false;

    let candidate = normalized;
    const bracketMatch = candidate.match(/^\[(.+)\](?::\d+)?$/);
    if (bracketMatch?.[1]) {
      candidate = bracketMatch[1];
    }

    if (candidate === "localhost" || candidate.startsWith("localhost:"))
      return true;
    if (candidate === "::1" || candidate === "0:0:0:0:0:0:0:1") return true;
    if (candidate.startsWith("fc") || candidate.startsWith("fd")) return true;
    if (candidate.startsWith("fe80:")) return true;
    if (/^127\.\d+\.\d+\.\d+(?::\d+)?$/.test(candidate)) return true;
    if (/^10\.\d+\.\d+\.\d+(?::\d+)?$/.test(candidate)) return true;
    if (/^192\.168\.\d+\.\d+(?::\d+)?$/.test(candidate)) return true;
    if (/^172\.(1[6-9]|2\d|3[0-1])\.\d+\.\d+(?::\d+)?$/.test(candidate))
      return true;

    const mappedIpv4Match = candidate.match(
      /^::ffff:(\d+\.\d+\.\d+\.\d+)(?::\d+)?$/,
    );
    if (mappedIpv4Match?.[1]) {
      const mappedIpv4 = mappedIpv4Match[1];
      if (mappedIpv4.startsWith("127.")) return true;
      if (mappedIpv4.startsWith("10.")) return true;
      if (mappedIpv4.startsWith("192.168.")) return true;
      if (/^172\.(1[6-9]|2\d|3[0-1])\./.test(mappedIpv4)) return true;
    }

    return false;
  }

  private async resolveIpLocation(ip: string): Promise<string> {
    return ipLocationService.getCachedLocation(ip);
  }

  private sanitizeIps(ips: string[]) {
    return [
      ...new Set(
        ips
          .filter((ip): ip is string => typeof ip === "string")
          .map((ip) => this.normalizeIp(ip))
          .filter(Boolean),
      ),
    ];
  }

  private async ensureMinimumTtl(key: string, ttlSeconds: number) {
    if (ttlSeconds <= 0) return;
    const currentTtl = await redis.ttl(key);
    if (currentTtl === -2 || currentTtl === -1 || currentTtl < ttlSeconds) {
      await redis.expire(key, ttlSeconds);
    }
  }

  private normalizePath(path: string) {
    const clean = path.split("?")[0]?.split("#")[0] ?? "";
    if (!clean) return "/";
    const normalized = clean.startsWith("/") ? clean : `/${clean}`;
    if (normalized.length > 1 && normalized.endsWith("/")) {
      return normalized.slice(0, -1);
    }
    return normalized;
  }

  private parsePath(value: string) {
    try {
      return new URL(value, "http://127.0.0.1");
    } catch {
      return null;
    }
  }

  private normalizeSubsonicRestEndpoint(path: string) {
    const match = this.normalizePath(path).match(
      /^\/rest\/([a-zA-Z][a-zA-Z0-9]*)(?:\.(?:view|json|xml))?$/,
    );
    return match?.[1]?.toLowerCase() || "";
  }

  private isKnownSubsonicRestPath(path: string) {
    const parsed = this.parsePath(path);
    if (!parsed) return false;

    const endpoint = this.normalizeSubsonicRestEndpoint(parsed.pathname);
    return Boolean(endpoint && SUBSONIC_REST_ENDPOINTS.has(endpoint));
  }

  private isKnownProxyPath(requestPath: string, mappingPath: string) {
    const cleanRequestPath = this.normalizePath(requestPath);
    const cleanMappingPath = this.normalizePath(mappingPath);

    if (cleanMappingPath === "/") return true;
    if (cleanRequestPath === cleanMappingPath) return true;
    return cleanRequestPath.startsWith(`${cleanMappingPath}/`);
  }

  private isFnosSharePath(path: string) {
    const cleanPath = this.normalizePath(path);
    return cleanPath === "/s" || cleanPath.startsWith("/s/");
  }

  private findBestMatchingProxyMapping(
    path: string,
    mappings: ProxyMapping[],
  ): ProxyMapping | null {
    const cleanPath = this.normalizePath(path);
    let bestMatch: ProxyMapping | null = null;
    let bestLength = -1;

    for (const mapping of mappings) {
      if (!mapping?.path || !this.isKnownProxyPath(cleanPath, mapping.path)) {
        continue;
      }
      const normalizedMappingPath = this.normalizePath(mapping.path);
      if (normalizedMappingPath.length <= bestLength) continue;
      bestMatch = mapping;
      bestLength = normalizedMappingPath.length;
    }

    return bestMatch;
  }

  private findMatchingHostMapping(host: string, mappings: HostMapping[]) {
    const cleanHost = this.normalizeHost(host);
    if (!cleanHost) return null;

    return (
      mappings.find(
        (mapping) => this.normalizeHost(mapping.host) === cleanHost,
      ) || null
    );
  }

  private isPublicHostMapping(mapping: HostMapping | null) {
    return Boolean(
      mapping &&
      mapping.use_auth === false &&
      mapping.access_mode !== "strict_whitelist",
    );
  }

  private resolveDefaultProxyMapping(
    config: Pick<AppConfig, "default_route" | "proxy_mappings">,
  ): ProxyMapping | null {
    const defaultRoute = config.default_route?.trim();
    if (!defaultRoute || defaultRoute === "/__select__") {
      return null;
    }

    return (
      config.proxy_mappings.find(
        (mapping) =>
          this.normalizePath(mapping.path) === this.normalizePath(defaultRoute),
      ) || null
    );
  }

  isRequestExemptFromScan(
    request: Request,
    config: Pick<
      AppConfig,
      | "run_type"
      | "reverse_proxy_submode"
      | "default_route"
      | "fnos_share_bypass"
      | "proxy_mappings"
      | "host_mappings"
    >,
  ) {
    const forwardedPath = this.resolveForwardedPath(request);
    if (
      config.fnos_share_bypass?.enabled === true &&
      this.isFnosSharePath(forwardedPath)
    ) {
      return true;
    }

    if (isAnySubdomainRoutingMode(config)) {
      const matchedHostMapping = this.findMatchingHostMapping(
        this.resolveForwardedHost(request),
        config.host_mappings || [],
      );
      return this.isPublicHostMapping(matchedHostMapping);
    }

    const matchedProxyMapping = this.findBestMatchingProxyMapping(
      forwardedPath,
      config.proxy_mappings || [],
    );
    if (matchedProxyMapping) {
      return matchedProxyMapping.use_auth === false;
    }

    const defaultProxyMapping = this.resolveDefaultProxyMapping(config);
    return defaultProxyMapping?.use_auth === false;
  }

  async isCommonPath(path: string) {
    const cleanPath = this.normalizePath(path);
    if (this.isKnownSubsonicRestPath(path)) return true;
    if (cleanPath === "/__auth__" || cleanPath.startsWith("/__auth__/"))
      return true;
    if (
      cleanPath === "/api/auth/passkey" ||
      cleanPath.startsWith("/api/auth/passkey/")
    )
      return true;
    if (cleanPath === "/websocket") return true;
    if (
      cleanPath === "/api/admin/terminal" ||
      cleanPath.startsWith("/api/admin/terminal/")
    )
      return true;
    if (
      cleanPath === "/cgi/ThirdParty" ||
      cleanPath.startsWith("/cgi/ThirdParty/")
    )
      return true;
    if (cleanPath === "/assets/" || cleanPath.startsWith("/assets/"))
      return true;
    if (cleanPath === "/s/" || cleanPath.startsWith("/s/")) return true;
    const common = new Set([
      "/",
      "/index.html",
      "/robots.txt",
      "/sitemap.xml",
      "/favicon.ico",
      "/favicon.svg",
      "/api/auth/bootstrap",
      "/api/auth/captcha/config",
      "/api/auth/challenge",
      "/api/auth/login",
      "/api/auth/ip",
      "/api/auth/ip/location",
      "/api/auth/session",
      "/api/auth/verify",
      "/api/auth/passkey/status",
      "/trimcon",
      "/.well-known/ai-plugin.json",
      "/apple-touch-icon.png",
      "/manifest.json",
      "/login",
      "/locales/zh-CN/os.json",
      "/license/v1/device/baseInfo",
      "/locales/zh-CN/apps/setting.json",
      "/app-center/v1/check-update?language=zh-CN",
      "/sac/rpcproxy/v1/new-user-guide/status",
      "/locales/zh-CN/pages/login.json",
      "/static/bg/wallpaper-1.webp",
      "/api/config",
      "/identity/connect/token",
    ]);
    if (common.has(cleanPath)) return true;

    const config = await configManager.getConfig();
    const proxyMappings = config.proxy_mappings || [];
    return proxyMappings.some((mapping) => {
      if (!mapping?.path) return false;
      return this.isKnownProxyPath(cleanPath, mapping.path);
    });
  }

  async getSettings(): Promise<ScannerSettings> {
    const envEnabledRaw = String(process.env.SCANNER_ENABLED ?? "")
      .trim()
      .toLowerCase();
    const envEnabled = envEnabledRaw === "true" || envEnabledRaw === "1";
    const envWindowMinutes = parseIntSafe(
      process.env.SCANNER_WINDOW_MINUTES,
      5,
    );
    const envThreshold = parseIntSafe(process.env.SCANNER_THRESHOLD, 5);
    const envBlacklistTtlDays = parseIntSafe(
      process.env.SCANNER_BLACKLIST_TTL_DAYS,
      90,
    );

    let enabled = envEnabled;
    let windowMinutes = envWindowMinutes;
    let threshold = envThreshold;
    let blacklistTtlSeconds = envBlacklistTtlDays * 24 * 3600;
    let commonLocationExemptEnabled = false;
    let cidrExemptions: string[] = [];
    let cidrExemptionRegions: ScannerCidrExemptionSelection[] = [];
    let cidrExemptionRegionCidrs: string[] = [];
    let cidrExemptionCidrs: string[] = [];

    const raw = await redis.get(this.settingsKey);
    if (raw) {
      try {
        const parsed = JSON.parse(raw) as Partial<ScannerSettings>;
        if (typeof parsed.enabled === "boolean") enabled = parsed.enabled;
        if (parsed.windowMinutes && parsed.windowMinutes > 0)
          windowMinutes = parsed.windowMinutes;
        if (parsed.threshold && parsed.threshold > 0)
          threshold = parsed.threshold;
        if (parsed.blacklistTtlSeconds && parsed.blacklistTtlSeconds > 0) {
          blacklistTtlSeconds = parsed.blacklistTtlSeconds;
        }
        if (typeof parsed.commonLocationExemptEnabled === "boolean") {
          commonLocationExemptEnabled = parsed.commonLocationExemptEnabled;
        }
        cidrExemptions = normalizeScannerCidrExemptions(
          parsed.cidrExemptions,
        );
        cidrExemptionRegions = normalizeScannerCidrExemptionRegions(
          parsed.cidrExemptionRegions,
        );
        cidrExemptionRegionCidrs = normalizeScannerCidrExemptions(
          parsed.cidrExemptionRegionCidrs,
        );
        cidrExemptionCidrs = normalizeScannerCidrExemptions(
          parsed.cidrExemptionCidrs,
        );
      } catch {}
    }

    const effectiveCidrExemptions =
      cidrExemptionCidrs.length > 0
        ? cidrExemptionCidrs
        : normalizeScannerCidrExemptions([
            ...cidrExemptionRegionCidrs,
            ...cidrExemptions,
          ]);
    const windowSeconds = Math.max(this.baseWindowSeconds, windowMinutes * 60);
    return {
      enabled,
      windowMinutes,
      threshold,
      windowSeconds,
      blacklistTtlSeconds,
      commonLocationExemptEnabled,
      cidrExemptions,
      cidrExemptionRegions,
      cidrExemptionRegionCidrs,
      cidrExemptionCidrs: effectiveCidrExemptions,
    };
  }

  private async resolveCidrExemptionRegions(value: unknown): Promise<{
    regions: ScannerCidrExemptionSelection[];
    cidrs: string[];
  }> {
    const inputs = dedupeScannerCidrExemptionRegionInputs(value);
    const regions: ScannerCidrExemptionSelection[] = [];
    const cidrs: string[] = [];

    for (const input of inputs) {
      const lookup = await cidrService.getCidrs({
        province: input.province,
        city: input.query_city,
      });
      regions.push(toStoredScannerCidrExemptionSelection(lookup.selection));
      cidrs.push(...lookup.cidrGroups.ipv4, ...lookup.cidrGroups.ipv6);
    }

    return {
      regions,
      cidrs: normalizeScannerCidrExemptions(cidrs),
    };
  }

  async updateSettings(payload: {
    enabled: boolean;
    windowMinutes: number;
    threshold: number;
    blacklistTtlSeconds: number;
    commonLocationExemptEnabled?: boolean;
    cidrExemptions?: string[];
    cidrExemptionRegions?: ScannerCidrExemptionRegionInput[];
  }): Promise<ScannerSettings> {
    const previous = await this.getSettings();
    const manualCidrs =
      payload.cidrExemptions === undefined
        ? previous.cidrExemptions
        : validateScannerCidrExemptions(payload.cidrExemptions);
    const requestedRegions =
      payload.cidrExemptionRegions === undefined
        ? null
        : dedupeScannerCidrExemptionRegionInputs(
            payload.cidrExemptionRegions,
          );
    const previousRegionInputs = previous.cidrExemptionRegions.map((item) => ({
      province: item.province,
      query_city: item.query_city,
    }));
    const shouldReuseRegionResolution =
      requestedRegions === null ||
      scannerCidrRegionKeysEqual(requestedRegions, previousRegionInputs);
    const regionResolution = shouldReuseRegionResolution
      ? {
          regions: previous.cidrExemptionRegions,
          cidrs: previous.cidrExemptionRegionCidrs,
        }
      : await this.resolveCidrExemptionRegions(requestedRegions);
    const effectiveCidrExemptions = normalizeScannerCidrExemptions([
      ...regionResolution.cidrs,
      ...manualCidrs,
    ]);
    const next = {
      enabled: payload.enabled,
      windowMinutes: Math.max(1, Math.floor(payload.windowMinutes)),
      threshold: Math.max(1, Math.floor(payload.threshold)),
      blacklistTtlSeconds: Math.max(
        60,
        Math.floor(payload.blacklistTtlSeconds),
      ),
      commonLocationExemptEnabled:
        payload.commonLocationExemptEnabled === true,
      cidrExemptions: manualCidrs,
      cidrExemptionRegions: regionResolution.regions,
      cidrExemptionRegionCidrs: regionResolution.cidrs,
      cidrExemptionCidrs: effectiveCidrExemptions,
    };
    await redis.set(this.settingsKey, JSON.stringify(next));
    return this.getSettings();
  }

  async isBlacklisted(ip: string): Promise<boolean> {
    const cleanIp = this.normalizeIp(ip);
    if (!cleanIp || this.isLocalAddress(cleanIp)) return false;

    const settings = await this.getSettings();
    if (!settings.enabled) return false;
    if (
      settings.commonLocationExemptEnabled &&
      (await isCommonAuthLocationExemptIP(cleanIp))
    ) {
      return false;
    }
    if (isScannerCidrExemptIp(cleanIp, settings.cidrExemptionCidrs)) {
      return false;
    }

    const exists = await redis.exists(this.blacklistDataKey(cleanIp));
    return exists === 1;
  }

  async recordUncommonPath(
    ip: string,
    path: string,
  ): Promise<{ hitCount: number; blocked: boolean }> {
    const cleanIp = this.normalizeIp(ip);
    if (!cleanIp || this.isLocalAddress(cleanIp)) {
      return { hitCount: 0, blocked: false };
    }

    const settings = await this.getSettings();
    if (!settings.enabled) {
      return { hitCount: 0, blocked: false };
    }
    if (
      settings.commonLocationExemptEnabled &&
      (await isCommonAuthLocationExemptIP(cleanIp))
    ) {
      return { hitCount: 0, blocked: false };
    }
    if (isScannerCidrExemptIp(cleanIp, settings.cidrExemptionCidrs)) {
      return { hitCount: 0, blocked: false };
    }

    const now = Date.now();
    const key = this.suspiciousKey(cleanIp);
    const cleanPath = this.normalizePath(path);
    const hit: ScanHit = { path: cleanPath, createdAt: now };
    const minScore = now - settings.windowSeconds * 1000;
    const windowMinScore = now - settings.windowMinutes * 60 * 1000;

    const pipeline = redis.pipeline();
    pipeline.zadd(key, now, JSON.stringify(hit));
    pipeline.zremrangebyscore(key, 0, minScore);
    pipeline.expire(key, settings.windowSeconds + 60);
    pipeline.zcount(key, windowMinScore, "+inf");
    const result = await pipeline.exec();
    const countValue = result?.[3]?.[1];
    const hitCount =
      typeof countValue === "number" ? countValue : Number(countValue ?? 0);

    if (hitCount >= settings.threshold) {
      const alreadyBlocked = await this.isBlacklisted(cleanIp);
      if (!alreadyBlocked) {
        const hitsRaw = await redis.zrangebyscore(key, windowMinScore, "+inf");
        const hits: ScanHit[] = [];
        for (const raw of hitsRaw) {
          try {
            const parsed = JSON.parse(raw) as ScanHit;
            if (parsed?.path && parsed?.createdAt) hits.push(parsed);
          } catch {}
        }
        const ipLocation = await this.resolveIpLocation(cleanIp);
        await this.addToBlacklist(
          {
            ip: cleanIp,
            blockedAt: now,
            windowMinutes: settings.windowMinutes,
            threshold: settings.threshold,
            hits,
            ...(ipLocation ? { ipLocation } : {}),
          },
          settings.blacklistTtlSeconds,
        );
        await ipLocationService.registerUsage(cleanIp, [
          ipLocationRefs.scannerBlacklist(cleanIp),
        ]);
        await emitScannerBlockedEvent({
          ip: cleanIp,
          blockedAt: now,
          windowMinutes: settings.windowMinutes,
          threshold: settings.threshold,
          hitCount,
          hits,
          ...(ipLocation ? { ipLocation } : {}),
        });
        return { hitCount, blocked: true };
      }
    }
    return { hitCount, blocked: false };
  }

  async listBlacklist(payload: {
    page: number;
    limit: number;
    search?: string;
  }) {
    const page = Math.max(1, Math.floor(payload.page || 1));
    const limit = Math.max(1, Math.min(Math.floor(payload.limit || 20), 200));
    const search = payload.search?.trim() || "";
    const start = (page - 1) * limit;
    const end = start + limit - 1;

    let ips: string[] = [];
    let total = 0;

    if (search) {
      const chunkSize = Math.max(200, limit * 5);
      let matchedCount = 0;
      let offset = 0;

      while (true) {
        const chunk = await redis.zrevrange(
          this.blacklistIndexKey,
          offset,
          offset + chunkSize - 1,
        );
        if (chunk.length === 0) break;
        offset += chunk.length;

        for (const ip of chunk) {
          if (!ip.includes(search)) continue;
          if (matchedCount >= start && ips.length < limit) {
            ips.push(ip);
          }
          matchedCount += 1;
        }
      }

      total = matchedCount;
    } else {
      total = await redis.zcard(this.blacklistIndexKey);
      if (total > 0) {
        ips = await redis.zrevrange(this.blacklistIndexKey, start, end);
      }
    }

    const items = await this.getBlacklistRecords(ips);
    return { total, items };
  }

  async getBlacklistRecord(ip: string): Promise<BlacklistRecord | null> {
    const raw = await redis.get(this.blacklistDataKey(ip));
    if (!raw) return null;
    try {
      const parsed = JSON.parse(raw) as BlacklistRecord;
      const record = { ...parsed, ip: parsed.ip || ip };
      await ipLocationService.hydrateIpLocationRecords([record], (item) =>
        ipLocationRefs.scannerBlacklist(item.ip),
      );
      return record;
    } catch {
      return null;
    }
  }

  async listBlacklistByRange(
    fromMs: number,
    toMs: number,
  ): Promise<Array<{ ip: string; blockedAt: number }>> {
    const pairs = await redis.zrangebyscore(
      this.blacklistIndexKey,
      fromMs,
      toMs,
      "WITHSCORES",
    );
    if (!pairs.length) return [];
    const items: Array<{ ip: string; blockedAt: number }> = [];
    for (let i = 0; i < pairs.length; i += 2) {
      const ip = pairs[i];
      const score = Number(pairs[i + 1]);
      if (!ip || !Number.isFinite(score)) continue;
      items.push({ ip, blockedAt: score });
    }
    return items;
  }

  async removeFromBlacklist(ip: string): Promise<void> {
    const [cleanIp] = this.sanitizeIps([ip]);
    if (!cleanIp) return;
    await this.clearIpState(cleanIp);
  }

  async removeManyFromBlacklist(ips: string[]): Promise<void> {
    const cleanIps = this.sanitizeIps(ips);
    if (cleanIps.length === 0) return;
    await this.clearIpState(...cleanIps);
  }

  private async clearIpState(...ips: string[]) {
    const cleanIps = this.sanitizeIps(ips);
    if (cleanIps.length === 0) return;
    const pipeline = redis.pipeline();
    for (const ip of cleanIps) {
      pipeline.del(this.blacklistDataKey(ip));
      pipeline.del(this.suspiciousKey(ip));
    }
    pipeline.zrem(this.blacklistIndexKey, ...cleanIps);
    await pipeline.exec();
  }

  private async getBlacklistRecords(ips: string[]): Promise<BlacklistRecord[]> {
    if (ips.length === 0) return [];
    const keys = ips.map((ip) => this.blacklistDataKey(ip));
    const raws = await redis.mget(keys);
    const records: BlacklistRecord[] = [];
    const missingIps: string[] = [];

    raws.forEach((raw, index) => {
      const ip = ips[index];
      if (!ip) return;
      if (!raw) {
        missingIps.push(ip);
        return;
      }
      try {
        const parsed = JSON.parse(raw) as BlacklistRecord;
        records.push({ ...parsed, ip: parsed.ip || ip });
      } catch {
        missingIps.push(ip);
      }
    });

    if (missingIps.length > 0) {
      await redis.zrem(this.blacklistIndexKey, ...missingIps);
    }

    await ipLocationService.hydrateIpLocationRecords(records, (record) =>
      ipLocationRefs.scannerBlacklist(record.ip),
    );
    return records;
  }

  private async addToBlacklist(record: BlacklistRecord, ttlSeconds: number) {
    if (!record.ip || this.isLocalAddress(record.ip)) return;

    const indexMinScore = record.blockedAt - ttlSeconds * 1000;
    const pipeline = redis.pipeline();
    pipeline.set(
      this.blacklistDataKey(record.ip),
      JSON.stringify(record),
      "EX",
      ttlSeconds,
    );
    pipeline.zadd(this.blacklistIndexKey, record.blockedAt, record.ip);
    pipeline.zremrangebyscore(this.blacklistIndexKey, 0, indexMinScore);
    await pipeline.exec();
    await this.ensureMinimumTtl(this.blacklistIndexKey, ttlSeconds);
  }
}

export const scanDetector = new ScanDetector();
