import { configManager, redis } from "./redis";
import { ipLocationService } from "./ip-location";

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
};

const parseIntSafe = (value: string | undefined, fallback: number) => {
  const v = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(v)) return fallback;
  return v;
};

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

  private isLocalAddress(ip: string) {
    const normalized = this.normalizeIp(ip).toLowerCase();
    if (!normalized) return false;

    let candidate = normalized;
    const bracketMatch = candidate.match(/^\[(.+)\](?::\d+)?$/);
    if (bracketMatch?.[1]) {
      candidate = bracketMatch[1];
    }

    if (candidate === "localhost" || candidate.startsWith("localhost:")) return true;
    if (candidate === "::1" || candidate === "0:0:0:0:0:0:0:1") return true;
    if (/^127\.\d+\.\d+\.\d+(?::\d+)?$/.test(candidate)) return true;

    const mappedIpv4Match = candidate.match(/^::ffff:(\d+\.\d+\.\d+\.\d+)(?::\d+)?$/);
    if (mappedIpv4Match?.[1]?.startsWith("127.")) return true;

    return false;
  }

  private async resolveIpLocation(ip: string): Promise<string> {
    try {
      const lookupIp = ip === "::1" ? "127.0.0.1" : ip;
      const info = await ipLocationService.getIpLocation(lookupIp);
      return info?.raw || "";
    } catch {
      return "";
    }
  }

  private sanitizeIps(ips: string[]) {
    return [...new Set(
      ips
        .filter((ip): ip is string => typeof ip === "string")
        .map((ip) => this.normalizeIp(ip))
        .filter(Boolean)
    )];
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

  private isKnownProxyPath(requestPath: string, mappingPath: string) {
    const cleanRequestPath = this.normalizePath(requestPath);
    const cleanMappingPath = this.normalizePath(mappingPath);

    if (cleanMappingPath === "/") return true;
    if (cleanRequestPath === cleanMappingPath) return true;
    return cleanRequestPath.startsWith(`${cleanMappingPath}/`);
  }

  async isCommonPath(path: string) {
    const cleanPath = this.normalizePath(path);
    if (cleanPath === "/__auth__" || cleanPath.startsWith("/__auth__/")) return true;
    if (cleanPath === "/api/auth/passkey" || cleanPath.startsWith("/api/auth/passkey/")) return true;
    if (cleanPath === "/websocket") return true;
    if (cleanPath === "/cgi/ThirdParty" || cleanPath.startsWith("/cgi/ThirdParty/")) return true;
    if (cleanPath === "/assets/" || cleanPath.startsWith("/assets/")) return true;
    if (cleanPath === "/s/" || cleanPath.startsWith("/s/")) return true;
    const common = new Set([
      "/",
      "/index.html",
      "/robots.txt",
      "/sitemap.xml",
      "/favicon.ico",
      "/favicon.svg",
      '/api/auth/ip',
      '/api/auth/verify',
      '/api/auth/passkey/status',
      '/trimcon',
      "/.well-known/ai-plugin.json",
      "/apple-touch-icon.png",
      "/manifest.json",
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
    const envEnabledRaw = String(process.env.SCANNER_ENABLED ?? "").trim().toLowerCase();
    const envEnabled = envEnabledRaw === "true" || envEnabledRaw === "1";
    const envWindowMinutes = parseIntSafe(process.env.SCANNER_WINDOW_MINUTES, 5);
    const envThreshold = parseIntSafe(process.env.SCANNER_THRESHOLD, 5);
    const envBlacklistTtlDays = parseIntSafe(process.env.SCANNER_BLACKLIST_TTL_DAYS, 90);

    let enabled = envEnabled;
    let windowMinutes = envWindowMinutes;
    let threshold = envThreshold;
    let blacklistTtlSeconds = envBlacklistTtlDays * 24 * 3600;

    const raw = await redis.get(this.settingsKey);
    if (raw) {
      try {
        const parsed = JSON.parse(raw) as Partial<ScannerSettings>;
        if (typeof parsed.enabled === "boolean") enabled = parsed.enabled;
        if (parsed.windowMinutes && parsed.windowMinutes > 0) windowMinutes = parsed.windowMinutes;
        if (parsed.threshold && parsed.threshold > 0) threshold = parsed.threshold;
        if (parsed.blacklistTtlSeconds && parsed.blacklistTtlSeconds > 0) {
          blacklistTtlSeconds = parsed.blacklistTtlSeconds;
        }
      } catch {}
    }

    const windowSeconds = Math.max(this.baseWindowSeconds, windowMinutes * 60);
    return {
      enabled,
      windowMinutes,
      threshold,
      windowSeconds,
      blacklistTtlSeconds,
    };
  }

  async updateSettings(payload: {
    enabled: boolean;
    windowMinutes: number;
    threshold: number;
    blacklistTtlSeconds: number;
  }): Promise<ScannerSettings> {
    const next = {
      enabled: payload.enabled,
      windowMinutes: Math.max(1, Math.floor(payload.windowMinutes)),
      threshold: Math.max(1, Math.floor(payload.threshold)),
      blacklistTtlSeconds: Math.max(60, Math.floor(payload.blacklistTtlSeconds)),
    };
    await redis.set(this.settingsKey, JSON.stringify(next));
    return this.getSettings();
  }

  async isBlacklisted(ip: string): Promise<boolean> {
    const cleanIp = this.normalizeIp(ip);
    if (!cleanIp || this.isLocalAddress(cleanIp)) return false;

    const [settings, exists] = await Promise.all([
      this.getSettings(),
      redis.exists(this.blacklistDataKey(cleanIp))
    ]);
    if (!settings.enabled) return false;

    return exists === 1;
  }

  async recordUncommonPath(ip: string, path: string): Promise<{ hitCount: number; blocked: boolean }> {
    const cleanIp = this.normalizeIp(ip);
    if (!cleanIp || this.isLocalAddress(cleanIp)) {
      return { hitCount: 0, blocked: false };
    }

    const settings = await this.getSettings();
    if (!settings.enabled) {
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
    const hitCount = typeof countValue === "number" ? countValue : Number(countValue ?? 0);

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
        await this.addToBlacklist({
          ip: cleanIp,
          blockedAt: now,
          windowMinutes: settings.windowMinutes,
          threshold: settings.threshold,
          hits,
          ...(ipLocation ? { ipLocation } : {}),
        }, settings.blacklistTtlSeconds);
        return { hitCount, blocked: true };
      }
    }
    return { hitCount, blocked: false };
  }

  async listBlacklist(payload: { page: number; limit: number; search?: string }) {
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
        const chunk = await redis.zrevrange(this.blacklistIndexKey, offset, offset + chunkSize - 1);
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
      return { ...parsed, ip: parsed.ip || ip };
    } catch {
      return null;
    }
  }

  async listBlacklistByRange(fromMs: number, toMs: number): Promise<Array<{ ip: string; blockedAt: number }>> {
    const pairs = await redis.zrangebyscore(this.blacklistIndexKey, fromMs, toMs, "WITHSCORES");
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

    return records;
  }

  private async addToBlacklist(record: BlacklistRecord, ttlSeconds: number) {
    if (!record.ip || this.isLocalAddress(record.ip)) return;

    const indexMinScore = record.blockedAt - ttlSeconds * 1000;
    const pipeline = redis.pipeline();
    pipeline.set(this.blacklistDataKey(record.ip), JSON.stringify(record), "EX", ttlSeconds);
    pipeline.zadd(this.blacklistIndexKey, record.blockedAt, record.ip);
    pipeline.zremrangebyscore(this.blacklistIndexKey, 0, indexMinScore);
    pipeline.expire(this.blacklistIndexKey, ttlSeconds);
    await pipeline.exec();
  }
}

export const scanDetector = new ScanDetector();
