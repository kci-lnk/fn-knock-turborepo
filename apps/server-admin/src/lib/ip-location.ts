import { isIP } from "node:net";
import { redis } from "./redis";

const LOOKUP_ENDPOINT =
  process.env.IP_LOOKUP_API_BASE ||
  "https://ipaddress.fnknock.cn/api/v1/ip/lookup";
const LOOKUP_TIMEOUT_MS = Math.max(
  2000,
  Number.parseInt(process.env.IP_LOOKUP_TIMEOUT_MS || "8000", 10) || 8000,
);
const SUCCESS_CACHE_TTL_SECONDS = 7 * 24 * 60 * 60;
const FAILED_STATE_TTL_SECONDS = 300;
const MAX_ATTEMPTS = 5;
const QUEUE_POLL_INTERVAL_MS = 1000;
const QUEUE_BATCH_SIZE = 3;
const PREFIX = "fn_knock:ip_location";

const KEYS = {
  queue: `${PREFIX}:queue`,
  cache: (ip: string) => `${PREFIX}:cache:${ip}`,
  state: (ip: string) => `${PREFIX}:state:${ip}`,
  refs: (ip: string) => `${PREFIX}:refs:${ip}`,
  lock: (ip: string) => `${PREFIX}:lock:${ip}`,
};

export type IpLocationLookupStatus =
  | "idle"
  | "queued"
  | "processing"
  | "success"
  | "failed"
  | "skipped";

export interface IpLocationResult {
  ip: string;
  normalizedIp: string;
  version: "ipv4" | "ipv6";
  continent: string;
  country: string;
  province: string;
  city: string;
  district: string;
  isp: string;
  countryCode: string;
  raw: string;
  sourceRaw: string;
}

export interface IpLocationSnapshot {
  ip: string;
  normalizedIp: string;
  status: IpLocationLookupStatus;
  attempts: number;
  maxAttempts: number;
  location: string;
  result?: IpLocationResult;
  error?: string;
  updatedAt: number;
}

type IpLocationState = {
  status: IpLocationLookupStatus;
  attempts: number;
  maxAttempts: number;
  updatedAt: number;
  error?: string;
  nextAttemptAt?: number;
  result?: IpLocationResult;
};

type LookupApiPayload = {
  code?: number;
  msg?: string;
  ip?: string;
  result?: {
    version?: string;
    continent?: string;
    country?: string;
    province?: string;
    city?: string;
    district?: string;
    isp?: string;
    country_code?: string;
    raw?: string;
  };
};

export const ipLocationRefs = {
  authLog: (id: string) => `auth-log|${id}`,
  whitelist: (id: string) => `whitelist|${id}`,
  session: (id: string) => `session|${id}`,
  sessionTimeline: (id: string) => `session-timeline|${id}`,
  scannerBlacklist: (ip: string) => `scanner-blacklist|${ip}`,
};

type IpLocationCarrier = {
  ip: string;
  ipLocation?: string;
};

type MobilityEventCarrier =
  | {
      kind: "login";
      toIp: string;
      toIpLocation?: string;
    }
  | {
      kind: "drift";
      fromIp: string;
      fromIpLocation?: string;
      toIp: string;
      toIpLocation?: string;
    };

class IpLocationService {
  private processing = false;
  private timer: ReturnType<typeof setInterval> | null = null;

  constructor() {
    this.timer = setInterval(() => {
      this.processQueue().catch((error) => {
        console.error("[ip-location] queue tick failed:", error);
      });
    }, QUEUE_POLL_INTERVAL_MS);
    this.timer.unref?.();
  }

  normalizeIp(ip: string): string {
    let candidate = String(ip || "").trim();
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
  }

  async getCachedLocation(ip: string): Promise<string> {
    const cached = await this.getCachedResult(ip);
    return cached?.raw || "";
  }

  async getCachedResult(ip: string): Promise<IpLocationResult | null> {
    const normalizedIp = this.normalizeIp(ip);
    if (!normalizedIp) return null;

    const raw = await redis.get(KEYS.cache(normalizedIp));
    if (!raw) return null;

    try {
      return JSON.parse(raw) as IpLocationResult;
    } catch {
      return null;
    }
  }

  async ensureEnqueued(ip: string): Promise<IpLocationSnapshot> {
    const normalizedIp = this.normalizeIp(ip);
    if (!normalizedIp) {
      return this.buildSnapshot(ip, "", {
        status: "skipped",
        attempts: 0,
        maxAttempts: MAX_ATTEMPTS,
        updatedAt: Date.now(),
        error: "invalid ip",
      });
    }

    const cached = await this.getCachedResult(normalizedIp);
    if (cached) {
      const state = this.buildSuccessState(cached, MAX_ATTEMPTS);
      await this.persistState(normalizedIp, state, SUCCESS_CACHE_TTL_SECONDS);
      return this.buildSnapshot(ip, normalizedIp, state);
    }

    if (this.isSkippableIp(normalizedIp)) {
      const state: IpLocationState = {
        status: "skipped",
        attempts: 0,
        maxAttempts: MAX_ATTEMPTS,
        updatedAt: Date.now(),
      };
      await this.persistState(normalizedIp, state, FAILED_STATE_TTL_SECONDS);
      return this.buildSnapshot(ip, normalizedIp, state);
    }

    const current = await this.getState(normalizedIp);
    if (current) {
      if (current.status === "success") {
        return this.buildSnapshot(ip, normalizedIp, current);
      }
      if (
        current.status === "queued" ||
        current.status === "processing" ||
        current.status === "failed"
      ) {
        return this.buildSnapshot(ip, normalizedIp, current);
      }
    }

    const nextState: IpLocationState = {
      status: "queued",
      attempts: current?.attempts ?? 0,
      maxAttempts: MAX_ATTEMPTS,
      updatedAt: Date.now(),
      nextAttemptAt: Date.now(),
    };

    const pipeline = redis.pipeline();
    pipeline.set(
      KEYS.state(normalizedIp),
      JSON.stringify(nextState),
      "EX",
      FAILED_STATE_TTL_SECONDS,
    );
    pipeline.zadd(KEYS.queue, Date.now(), normalizedIp);
    await pipeline.exec();

    return this.buildSnapshot(ip, normalizedIp, nextState);
  }

  async registerUsage(ip: string, references: string[] = []): Promise<string> {
    const normalizedIp = this.normalizeIp(ip);
    if (!normalizedIp) return "";

    const uniqueReferences = [...new Set(references.filter(Boolean))];
    if (uniqueReferences.length > 0) {
      await redis.sadd(KEYS.refs(normalizedIp), ...uniqueReferences);
    }

    const cached = await this.getCachedResult(normalizedIp);
    if (cached) {
      if (uniqueReferences.length > 0) {
        await Promise.all(
          uniqueReferences.map((reference) => this.syncReference(reference, cached)),
        );
      }
      return cached.raw;
    }

    await this.ensureEnqueued(normalizedIp);
    return "";
  }

  async hydrateIpLocationRecords<T extends IpLocationCarrier>(
    items: T[],
    getReference?: (item: T) => string | undefined,
  ): Promise<T[]> {
    if (items.length === 0) return items;

    const refsByIp = new Map<string, string[]>();
    const uniqueIps = new Set<string>();

    for (const item of items) {
      const normalizedIp = this.normalizeIp(item.ip);
      if (!normalizedIp) continue;
      uniqueIps.add(normalizedIp);

      const reference = getReference?.(item);
      if (!reference) continue;

      const refs = refsByIp.get(normalizedIp) || [];
      refs.push(reference);
      refsByIp.set(normalizedIp, refs);
    }

    await Promise.all(
      [...uniqueIps].map((ip) => this.registerUsage(ip, refsByIp.get(ip) || [])),
    );

    const cachedMap = await this.getCachedLocationMap([...uniqueIps]);
    for (const item of items) {
      const normalizedIp = this.normalizeIp(item.ip);
      if (!normalizedIp) continue;

      const location = cachedMap.get(normalizedIp);
      if (location) {
        item.ipLocation = location;
      }
    }

    return items;
  }

  async hydrateMobilityEvents<T extends MobilityEventCarrier>(
    events: T[],
    sessionId: string,
  ): Promise<T[]> {
    if (events.length === 0) return events;

    const reference = ipLocationRefs.sessionTimeline(sessionId);
    const uniqueIps = new Set<string>();
    for (const event of events) {
      if ("toIp" in event) {
        const normalizedToIp = this.normalizeIp(event.toIp);
        if (normalizedToIp) uniqueIps.add(normalizedToIp);
      }
      if ("fromIp" in event) {
        const normalizedFromIp = this.normalizeIp(event.fromIp);
        if (normalizedFromIp) uniqueIps.add(normalizedFromIp);
      }
    }

    await Promise.all(
      [...uniqueIps].map((ip) => this.registerUsage(ip, [reference])),
    );

    const cachedMap = await this.getCachedLocationMap([...uniqueIps]);
    for (const event of events) {
      const toIp = this.normalizeIp(event.toIp);
      if (toIp) {
        const toLocation = cachedMap.get(toIp);
        if (toLocation) {
          event.toIpLocation = toLocation;
        }
      }

      if ("fromIp" in event) {
        const fromIp = this.normalizeIp(event.fromIp);
        if (fromIp) {
          const fromLocation = cachedMap.get(fromIp);
          if (fromLocation) {
            event.fromIpLocation = fromLocation;
          }
        }
      }
    }

    return events;
  }

  private async getCachedLocationMap(ips: string[]): Promise<Map<string, string>> {
    const normalizedIps = [...new Set(ips.map((ip) => this.normalizeIp(ip)).filter(Boolean))];
    if (normalizedIps.length === 0) {
      return new Map();
    }

    const rawResults = await redis.mget(normalizedIps.map((ip) => KEYS.cache(ip)));
    const map = new Map<string, string>();

    rawResults.forEach((raw, index) => {
      const ip = normalizedIps[index];
      if (!ip || !raw) return;
      try {
        const parsed = JSON.parse(raw) as IpLocationResult;
        if (parsed.raw) {
          map.set(ip, parsed.raw);
        }
      } catch {
        // ignore invalid cache
      }
    });

    return map;
  }

  private async processQueue(): Promise<void> {
    if (this.processing) return;

    this.processing = true;
    try {
      const dueIps = await redis.zrangebyscore(
        KEYS.queue,
        0,
        Date.now(),
        "LIMIT",
        0,
        QUEUE_BATCH_SIZE,
      );

      if (dueIps.length === 0) return;

      await Promise.all(dueIps.map((ip) => this.processOne(ip)));
    } finally {
      this.processing = false;
    }
  }

  private async processOne(ip: string): Promise<void> {
    const lockTtlSeconds = Math.max(10, Math.ceil(LOOKUP_TIMEOUT_MS / 1000) + 5);
    const locked = await redis.set(KEYS.lock(ip), String(Date.now()), "EX", lockTtlSeconds, "NX");
    if (locked !== "OK") return;

    try {
      await redis.zrem(KEYS.queue, ip);
      const state = await this.getState(ip);
      const attempts = state?.attempts ?? 0;

      if (attempts >= MAX_ATTEMPTS) {
        await this.persistState(
          ip,
          {
            status: "failed",
            attempts,
            maxAttempts: MAX_ATTEMPTS,
            updatedAt: Date.now(),
            error: state?.error || "max attempts reached",
          },
          FAILED_STATE_TTL_SECONDS,
        );
        return;
      }

      await this.persistState(
        ip,
        {
          status: "processing",
          attempts,
          maxAttempts: MAX_ATTEMPTS,
          updatedAt: Date.now(),
        },
        FAILED_STATE_TTL_SECONDS,
      );

      const nextAttempt = attempts + 1;
      const lookupResult = await this.lookupRemote(ip);

      if (lookupResult.ok) {
        const successState = this.buildSuccessState(lookupResult.result, nextAttempt);
        const pipeline = redis.pipeline();
        pipeline.set(
          KEYS.cache(ip),
          JSON.stringify(lookupResult.result),
          "EX",
          SUCCESS_CACHE_TTL_SECONDS,
        );
        pipeline.set(
          KEYS.state(ip),
          JSON.stringify(successState),
          "EX",
          SUCCESS_CACHE_TTL_SECONDS,
        );
        pipeline.zrem(KEYS.queue, ip);
        await pipeline.exec();

        await this.syncReferences(ip, lookupResult.result);
        return;
      }

      const error = lookupResult.error || "lookup failed";
      if (nextAttempt >= MAX_ATTEMPTS) {
        await this.persistState(
          ip,
          {
            status: "failed",
            attempts: nextAttempt,
            maxAttempts: MAX_ATTEMPTS,
            updatedAt: Date.now(),
            error,
          },
          FAILED_STATE_TTL_SECONDS,
        );
        return;
      }

      const nextAttemptAt = Date.now() + this.retryDelayMs(nextAttempt);
      const pipeline = redis.pipeline();
      pipeline.set(
        KEYS.state(ip),
        JSON.stringify({
          status: "queued",
          attempts: nextAttempt,
          maxAttempts: MAX_ATTEMPTS,
          updatedAt: Date.now(),
          error,
          nextAttemptAt,
        } satisfies IpLocationState),
        "EX",
        FAILED_STATE_TTL_SECONDS,
      );
      pipeline.zadd(KEYS.queue, nextAttemptAt, ip);
      await pipeline.exec();
    } catch (error) {
      console.error(`[ip-location] failed to process ${ip}:`, error);
    } finally {
      await redis.del(KEYS.lock(ip));
    }
  }

  private async lookupRemote(
    ip: string,
  ): Promise<{ ok: true; result: IpLocationResult } | { ok: false; error: string }> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), LOOKUP_TIMEOUT_MS);

    try {
      const url = new URL(LOOKUP_ENDPOINT);
      url.searchParams.set("ip", ip);

      const response = await fetch(url, {
        method: "GET",
        signal: controller.signal,
      });

      if (!response.ok) {
        return {
          ok: false,
          error: `http ${response.status}`,
        };
      }

      const payload = (await response.json().catch(() => null)) as LookupApiPayload | null;
      if (!payload || payload.code !== 0 || !payload.result) {
        return {
          ok: false,
          error: payload?.msg || "invalid lookup response",
        };
      }

      const result = this.toLocationResult(ip, payload);
      if (!result) {
        return {
          ok: false,
          error: "empty lookup result",
        };
      }

      return { ok: true, result };
    } catch (error: any) {
      if (error?.name === "AbortError") {
        return {
          ok: false,
          error: "lookup timeout",
        };
      }

      return {
        ok: false,
        error: error?.message || "lookup request failed",
      };
    } finally {
      clearTimeout(timeout);
    }
  }

  private toLocationResult(ip: string, payload: LookupApiPayload): IpLocationResult | null {
    const normalizedIp = this.normalizeIp(ip);
    if (!normalizedIp || !payload.result) return null;

    const version = payload.result.version === "ipv6" ? "ipv6" : "ipv4";
    const continent = String(payload.result.continent || "").trim();
    const country = String(payload.result.country || "").trim();
    const province = String(payload.result.province || "").trim();
    const city = String(payload.result.city || "").trim();
    const district = String(payload.result.district || "").trim();
    const isp = String(payload.result.isp || "").trim();
    const countryCode = String(payload.result.country_code || "").trim();
    const sourceRaw = String(payload.result.raw || "").trim();
    const raw = this.formatRaw({
      country,
      province,
      city,
      isp,
      sourceRaw,
    });

    if (!raw) return null;

    return {
      ip,
      normalizedIp,
      version,
      continent,
      country,
      province,
      city,
      district,
      isp,
      countryCode,
      raw,
      sourceRaw,
    };
  }

  private formatRaw(input: {
    country: string;
    province: string;
    city: string;
    isp: string;
    sourceRaw: string;
  }): string {
    const country = input.country.trim();
    const province = input.province.trim();
    const city = input.city.trim();
    const isp = input.isp.trim();

    if (country === "中国") {
      if (this.isSpecialChinaRegion(province)) {
        return [province, city, isp].filter(Boolean).join("|");
      }

      return [province || country, city, isp].filter(Boolean).join("|");
    }

    const foreign = [country, province, city, isp].filter(Boolean).join("|");
    return foreign || input.sourceRaw.trim();
  }

  private isSpecialChinaRegion(province: string): boolean {
    return new Set(["台湾", "香港", "澳门"]).has(province.trim());
  }

  private isSkippableIp(ip: string): boolean {
    if (isIP(ip) === 4) {
      if (ip === "0.0.0.0") return true;
      if (ip.startsWith("127.")) return true;
      if (ip.startsWith("10.")) return true;
      if (ip.startsWith("192.168.")) return true;
      if (/^172\.(1[6-9]|2\d|3[0-1])\./.test(ip)) return true;
      return false;
    }

    const lower = ip.toLowerCase();
    if (lower === "::" || lower === "::1") return true;
    if (lower.startsWith("fc") || lower.startsWith("fd")) return true;
    if (lower.startsWith("fe80:")) return true;
    return false;
  }

  private retryDelayMs(attempt: number): number {
    if (attempt <= 1) return 2000;
    return 5000;
  }

  private buildSuccessState(result: IpLocationResult, attempts: number): IpLocationState {
    return {
      status: "success",
      attempts,
      maxAttempts: MAX_ATTEMPTS,
      updatedAt: Date.now(),
      result,
    };
  }

  private buildSnapshot(
    ip: string,
    normalizedIp: string,
    state: IpLocationState,
  ): IpLocationSnapshot {
    return {
      ip,
      normalizedIp,
      status: state.status,
      attempts: state.attempts,
      maxAttempts: state.maxAttempts,
      location: state.result?.raw || "",
      result: state.result,
      error: state.error,
      updatedAt: state.updatedAt,
    };
  }

  private async getState(ip: string): Promise<IpLocationState | null> {
    const normalizedIp = this.normalizeIp(ip);
    if (!normalizedIp) return null;

    const raw = await redis.get(KEYS.state(normalizedIp));
    if (!raw) return null;

    try {
      return JSON.parse(raw) as IpLocationState;
    } catch {
      return null;
    }
  }

  private async persistState(ip: string, state: IpLocationState, ttlSeconds: number): Promise<void> {
    await redis.set(KEYS.state(ip), JSON.stringify(state), "EX", ttlSeconds);
  }

  private async syncReferences(ip: string, result: IpLocationResult): Promise<void> {
    const refs = await redis.smembers(KEYS.refs(ip));
    if (refs.length === 0) return;

    await Promise.all(refs.map((reference) => this.syncReference(reference, result)));
  }

  private async syncReference(reference: string, result: IpLocationResult): Promise<void> {
    const [type, id] = reference.split("|", 2);
    if (!type || !id) return;

    switch (type) {
      case "auth-log":
        await this.syncRedisJsonKey(`fn_knock:auth_log_data:${id}`, result);
        return;
      case "session":
        await this.syncRedisJsonKey(`fn_knock:session:${id}`, result);
        return;
      case "whitelist":
        await this.syncRedisHashRecord("fn_knock:whitelist:records", id, result);
        await this.syncRedisHashRecord("fn_knock:whitelist:deleted", id, result);
        return;
      case "scanner-blacklist":
        await this.syncRedisJsonKey(`fn_knock:scanner:blacklist:data:${id}`, result);
        return;
      case "session-timeline":
        await this.syncSessionTimeline(id, result);
        return;
      default:
        return;
    }
  }

  private async syncRedisHashRecord(
    key: string,
    field: string,
    result: IpLocationResult,
  ): Promise<void> {
    const raw = await redis.hget(key, field);
    if (!raw) return;

    try {
      const parsed = JSON.parse(raw) as Record<string, unknown>;
      if (!this.recordMatchesIp(parsed, result.normalizedIp, "ip")) return;
      parsed.ipLocation = result.raw;
      await redis.hset(key, field, JSON.stringify(parsed));
    } catch {
      // ignore invalid payload
    }
  }

  private async syncRedisJsonKey(key: string, result: IpLocationResult): Promise<void> {
    const [raw, ttl] = await Promise.all([redis.get(key), redis.ttl(key)]);
    if (!raw) return;

    try {
      const parsed = JSON.parse(raw) as Record<string, unknown>;
      if (!this.recordMatchesIp(parsed, result.normalizedIp, "ip")) return;
      parsed.ipLocation = result.raw;

      if (ttl > 0) {
        await redis.set(key, JSON.stringify(parsed), "EX", ttl);
      } else {
        await redis.set(key, JSON.stringify(parsed));
      }
    } catch {
      // ignore invalid payload
    }
  }

  private async syncSessionTimeline(sessionId: string, result: IpLocationResult): Promise<void> {
    const key = `fn_knock:auth_mobility:timeline:${sessionId}`;
    const [raw, ttl] = await Promise.all([redis.get(key), redis.ttl(key)]);
    if (!raw) return;

    try {
      const parsed = JSON.parse(raw) as Array<Record<string, unknown>>;
      if (!Array.isArray(parsed)) return;

      let updated = false;
      const nextEvents = parsed.map((event) => {
        if (!event || typeof event !== "object") return event;

        const next = { ...event };
        const toIp = typeof next.toIp === "string" ? this.normalizeIp(next.toIp) : "";
        if (toIp && toIp === result.normalizedIp && next.toIpLocation !== result.raw) {
          next.toIpLocation = result.raw;
          updated = true;
        }

        const fromIp =
          typeof next.fromIp === "string" ? this.normalizeIp(next.fromIp) : "";
        if (
          fromIp &&
          fromIp === result.normalizedIp &&
          next.fromIpLocation !== result.raw
        ) {
          next.fromIpLocation = result.raw;
          updated = true;
        }

        return next;
      });

      if (!updated) return;

      if (ttl > 0) {
        await redis.set(key, JSON.stringify(nextEvents), "EX", ttl);
      } else {
        await redis.set(key, JSON.stringify(nextEvents));
      }
    } catch {
      // ignore invalid payload
    }
  }

  private recordMatchesIp(
    record: Record<string, unknown>,
    normalizedIp: string,
    field: string,
  ): boolean {
    const rawValue = record[field];
    if (typeof rawValue !== "string") return false;
    return this.normalizeIp(rawValue) === normalizedIp;
  }
}

export const ipLocationService = new IpLocationService();
