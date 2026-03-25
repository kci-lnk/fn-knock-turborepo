import { randomBytes } from "node:crypto";
import { setTimeout as sleep } from "node:timers/promises";
import { isAuthServiceMapping, parseTargetPort } from "./auth-service";
import {
  buildFnosShareSessionClearCookie,
  buildFnosShareSessionCookie,
  FNOS_SHARE_SESSION_COOKIE_NAME,
} from "./session-cookie";
import {
  configManager,
  redis,
  type AppConfig,
  type FnosShareBypassConfig,
} from "./redis";
import { isAnySubdomainRoutingMode } from "./reverse-proxy-submode";

type ShareValidationCacheRecord = {
  version: 1;
  valid: boolean;
  validationState: "valid" | "invalid" | "unknown";
  shareId: string;
  cleanPath: string;
  token: string | null;
  name: string | null;
  type: number | null;
  checkedAt: string;
};

type ShareSessionRecord = {
  version: 1;
  shareId: string;
  cleanPath: string;
  token: string | null;
  name: string | null;
  type: number | null;
  issuedAt: string;
  lastSeenAt: string;
};

type ShareValidationFetchResult = {
  cacheable: boolean;
  data: ShareValidationCacheRecord;
};

export type FnosSharePreflightDecision = {
  handled: boolean;
  redirectLocation?: string;
};

export type FnosShareAuthorizationResult =
  | {
      authorized: true;
      setCookies: string[];
      responseHeaders: Record<string, string>;
    }
  | {
      authorized: false;
      setCookies?: string[];
      responseHeaders?: Record<string, string>;
    };

const SHARE_ENTRY_REGEX = /^\/s\/([a-z0-9]{18})$/;
const SHARE_SCRIPT_REGEX =
  /<script\b[^>]*\bid=(["'])share-data\1[^>]*>([\s\S]*?)<\/script>/i;
const CACHE_KEY_PREFIX = "fn_knock:fnos-share:validation:";
const SESSION_KEY_PREFIX = "fn_knock:fnos-share:session:";
const LOCK_KEY_PREFIX = "fn_knock:lock:fnos-share:validation:";
const FNOS_PRIMARY_PORT = 5666;
const FNOS_LEGACY_PORT = 8000;

const parseCookie = (cookieHeader: string, name: string): string | null => {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = cookieHeader.match(new RegExp(`(?:^|;\\s*)${escaped}=([^;]+)`));
  if (!match?.[1]) return null;
  const raw = match[1].trim().replace(/^"|"$/g, "");
  if (!raw) return null;
  try {
    return decodeURIComponent(raw);
  } catch {
    return raw;
  }
};

const parseRequestUrl = (rawPath: string | null): URL | null => {
  if (!rawPath || !rawPath.startsWith("/")) return null;
  try {
    return new URL(rawPath, "http://127.0.0.1");
  } catch {
    return null;
  }
};

const extractShareEntry = (
  requestUrl: URL,
): { shareId: string; cleanPath: string; isClean: boolean } | null => {
  const match = requestUrl.pathname.match(SHARE_ENTRY_REGEX);
  if (!match?.[1]) return null;

  const shareId = match[1];
  const cleanPath = `/s/${shareId}`;
  return {
    shareId,
    cleanPath,
    isClean: requestUrl.pathname === cleanPath && requestUrl.search === "",
  };
};

const isSharePath = (requestUrl: URL): boolean =>
  requestUrl.pathname === "/s" || requestUrl.pathname.startsWith("/s/");

const isSessionResourcePath = (
  pathname: string,
  cleanPath: string,
  shareId: string,
): boolean => {
  const previewPath = `/s/preview/${shareId}`;
  const thumbPath = `/s/thumb/${shareId}`;
  if (pathname.startsWith(`${cleanPath}/`)) return true;
  if (pathname.startsWith("/s/static/")) return true;
  if (pathname === previewPath || pathname.startsWith(`${previewPath}/`))
    return true;
  if (pathname === thumbPath || pathname.startsWith(`${thumbPath}/`))
    return true;
  return false;
};

const getRequestTarget = (request: Request): string => {
  const forwardedPath = request.headers.get("x-forwarded-path");
  if (forwardedPath) return forwardedPath;

  const currentUrl = new URL(request.url);
  return `${currentUrl.pathname}${currentUrl.search}`;
};

const toSessionKey = (sessionId: string) => `${SESSION_KEY_PREFIX}${sessionId}`;
const toCacheKey = (shareId: string) => `${CACHE_KEY_PREFIX}${shareId}`;
const toLockKey = (shareId: string) => `${LOCK_KEY_PREFIX}${shareId}`;

const normalizeShareValidation = (
  value: ShareValidationCacheRecord,
): ShareValidationCacheRecord => ({
  version: 1,
  valid: value.valid === true,
  validationState:
    value.validationState === "valid" ||
    value.validationState === "invalid" ||
    value.validationState === "unknown"
      ? value.validationState
      : value.valid === true
        ? "valid"
        : "invalid",
  shareId: value.shareId,
  cleanPath: value.cleanPath,
  token:
    typeof value.token === "string" && value.token.trim() ? value.token : null,
  name: typeof value.name === "string" && value.name.trim() ? value.name : null,
  type: typeof value.type === "number" ? value.type : null,
  checkedAt: value.checkedAt,
});

type ResolvedFnosShareBypassConfig = {
  policy: FnosShareBypassConfig;
  upstreamBaseUrl: URL | null;
  defaultRoute: string | null;
  matchedTarget: string | null;
};

const toUpstreamOrigin = (target: string): URL | null => {
  try {
    const targetUrl = new URL(target);
    if (targetUrl.protocol !== "http:" && targetUrl.protocol !== "https:") {
      return null;
    }
    return new URL(targetUrl.origin);
  } catch {
    return null;
  }
};

const scoreFnosHostMapping = (
  mapping: Pick<AppConfig["host_mappings"][number], "host" | "target">,
): number => {
  const port = parseTargetPort(mapping.target);
  const normalizedHost = mapping.host.trim().toLowerCase();

  if (port === FNOS_PRIMARY_PORT) return 100;
  if (port === FNOS_LEGACY_PORT) return 90;
  if (normalizedHost.includes("fnos")) return 20;
  return 0;
};

class FnosShareBypassService {
  private async getConfig(): Promise<ResolvedFnosShareBypassConfig> {
    const appConfig = await configManager.getConfig();
    const fallbackHostMapping = this.resolveFnosHostMapping(appConfig);
    const isSubdomainRouting = isAnySubdomainRoutingMode(appConfig);
    const defaultRoute = isSubdomainRouting
      ? null
      : appConfig.default_route || null;
    const matchedMapping = defaultRoute
      ? appConfig.proxy_mappings.find((item) => item.path === defaultRoute)
      : null;
    const matchedTarget = isSubdomainRouting
      ? fallbackHostMapping?.target || null
      : matchedMapping?.target || fallbackHostMapping?.target || null;
    return {
      policy:
        appConfig.fnos_share_bypass ??
        (await configManager.getFnosShareBypassConfig()),
      upstreamBaseUrl: this.resolveUpstreamBaseUrl(appConfig),
      defaultRoute,
      matchedTarget,
    };
  }

  private resolveUpstreamBaseUrl(appConfig: AppConfig): URL | null {
    if (!isAnySubdomainRoutingMode(appConfig)) {
      const defaultRoute = appConfig.default_route;
      if (defaultRoute && defaultRoute !== "/__select__") {
        const mapping = appConfig.proxy_mappings.find(
          (item) => item.path === defaultRoute,
        );
        if (mapping?.target) {
          const upstream = toUpstreamOrigin(mapping.target);
          if (upstream) return upstream;
        }
      }
    }

    const hostMapping = this.resolveFnosHostMapping(appConfig);
    return hostMapping?.target ? toUpstreamOrigin(hostMapping.target) : null;
  }

  private resolveFnosHostMapping(
    appConfig: Pick<AppConfig, "host_mappings">,
  ): AppConfig["host_mappings"][number] | null {
    const candidates = appConfig.host_mappings
      .filter((mapping) => mapping.target && !isAuthServiceMapping(mapping))
      .map((mapping) => ({
        mapping,
        score: scoreFnosHostMapping(mapping),
      }))
      .filter((item) => item.score > 0)
      .sort((a, b) => b.score - a.score);

    return candidates[0]?.mapping ?? null;
  }

  async resolvePreflight(
    request: Request,
  ): Promise<FnosSharePreflightDecision> {
    const config = await this.getConfig();
    if (!config.policy.enabled) {
      return { handled: false };
    }

    const requestUrl = parseRequestUrl(getRequestTarget(request));
    if (!requestUrl || !isSharePath(requestUrl)) {
      return { handled: false };
    }

    const shareEntry = extractShareEntry(requestUrl);
    if (shareEntry) {
      const validation = await this.validateShareLink(
        shareEntry.shareId,
        config,
      );
      if (!validation.valid) {
        return {
          handled: true,
          redirectLocation: "/",
        };
      }
      if (validation.valid && shareEntry.isClean) {
        return {
          handled: true,
        };
      }
      if (validation.valid && !shareEntry.isClean) {
        return {
          handled: true,
          redirectLocation: shareEntry.cleanPath,
        };
      }
    }

    const shareSessionId = parseCookie(
      request.headers.get("cookie") || "",
      FNOS_SHARE_SESSION_COOKIE_NAME,
    );
    const currentSession = shareSessionId
      ? await this.getShareSession(shareSessionId)
      : null;
    if (
      currentSession &&
      isSessionResourcePath(
        requestUrl.pathname,
        currentSession.cleanPath,
        currentSession.shareId,
      )
    ) {
      return { handled: true };
    }

    return {
      handled: true,
      redirectLocation: "/",
    };
  }

  async authorize(request: Request): Promise<FnosShareAuthorizationResult> {
    const config = await this.getConfig();
    if (!config.policy.enabled) {
      return { authorized: false };
    }

    const requestUrl = parseRequestUrl(getRequestTarget(request));
    if (!requestUrl || !isSharePath(requestUrl)) {
      return { authorized: false };
    }

    const cookieHeader = request.headers.get("cookie") || "";
    const shareSessionId = parseCookie(
      cookieHeader,
      FNOS_SHARE_SESSION_COOKIE_NAME,
    );
    const currentSession = shareSessionId
      ? await this.getShareSession(shareSessionId)
      : null;

    const shareEntry = extractShareEntry(requestUrl);
    if (shareEntry) {
      const validation = await this.validateShareLink(
        shareEntry.shareId,
        config,
      );
      if (!validation.valid) {
        return shareSessionId
          ? {
              authorized: false,
              setCookies: [buildFnosShareSessionClearCookie()],
              responseHeaders: { "X-Reauth-Redirect-Location": "/" },
            }
          : {
              authorized: false,
              responseHeaders: { "X-Reauth-Redirect-Location": "/" },
            };
      }

      if (currentSession && currentSession.shareId === validation.shareId) {
        await this.saveShareSession(
          shareSessionId!,
          currentSession,
          config.policy.session_ttl_seconds,
        );
        return {
          authorized: true,
          setCookies: [
            buildFnosShareSessionCookie(
              shareSessionId!,
              config.policy.session_ttl_seconds,
            ),
          ],
          responseHeaders: {
            "X-Reauth-Access-Mode": "fnos-share",
          },
        };
      }

      const sessionId = randomBytes(18).toString("hex");
      const session: ShareSessionRecord = {
        version: 1,
        shareId: validation.shareId,
        cleanPath: validation.cleanPath,
        token: validation.token,
        name: validation.name,
        type: validation.type,
        issuedAt: new Date().toISOString(),
        lastSeenAt: new Date().toISOString(),
      };
      await this.saveShareSession(
        sessionId,
        session,
        config.policy.session_ttl_seconds,
      );

      return {
        authorized: true,
        setCookies: [
          buildFnosShareSessionCookie(
            sessionId,
            config.policy.session_ttl_seconds,
          ),
        ],
        responseHeaders: {
          "X-Reauth-Access-Mode": "fnos-share",
        },
      };
    }

    if (!currentSession) {
      return shareSessionId
        ? {
            authorized: false,
            setCookies: [buildFnosShareSessionClearCookie()],
            responseHeaders: { "X-Reauth-Redirect-Location": "/" },
          }
        : {
            authorized: false,
            responseHeaders: { "X-Reauth-Redirect-Location": "/" },
          };
    }

    if (
      !isSessionResourcePath(
        requestUrl.pathname,
        currentSession.cleanPath,
        currentSession.shareId,
      )
    ) {
      return {
        authorized: false,
        setCookies: [buildFnosShareSessionClearCookie()],
        responseHeaders: { "X-Reauth-Redirect-Location": "/" },
      };
    }

    await this.saveShareSession(
      shareSessionId!,
      currentSession,
      config.policy.session_ttl_seconds,
    );

    return {
      authorized: true,
      setCookies: [
        buildFnosShareSessionCookie(
          shareSessionId!,
          config.policy.session_ttl_seconds,
        ),
      ],
      responseHeaders: {
        "X-Reauth-Access-Mode": "fnos-share",
      },
    };
  }

  async isSharePathRequest(request: Request): Promise<boolean> {
    const config = await this.getConfig();
    if (!config.policy.enabled) return false;

    const requestUrl = parseRequestUrl(getRequestTarget(request));
    return !!requestUrl && isSharePath(requestUrl);
  }

  private async validateShareLink(
    shareId: string,
    config: ResolvedFnosShareBypassConfig,
  ): Promise<ShareValidationCacheRecord> {
    const cacheKey = toCacheKey(shareId);
    const cached = await this.getCachedValidation(cacheKey);
    if (cached) return cached;

    const lockToken = await this.acquireLock(
      toLockKey(shareId),
      config.policy.validation_lock_ttl_seconds,
    );
    if (!lockToken) {
      const waited = await this.waitForCachedValidation(
        cacheKey,
        Math.min(
          config.policy.validation_lock_ttl_seconds * 1000,
          config.policy.upstream_timeout_ms + 500,
        ),
      );
      if (waited) return waited;
      const fallback = await this.fetchValidation(shareId, config);
      if (fallback.cacheable) {
        await this.cacheValidation(
          cacheKey,
          fallback.data,
          config.policy.validation_cache_ttl_seconds,
        );
      }
      return fallback.data;
    }

    try {
      const fresh = await this.fetchValidation(shareId, config);
      if (fresh.cacheable) {
        await this.cacheValidation(
          cacheKey,
          fresh.data,
          config.policy.validation_cache_ttl_seconds,
        );
      }
      return fresh.data;
    } finally {
      await this.releaseLock(toLockKey(shareId), lockToken);
    }
  }

  private async fetchValidation(
    shareId: string,
    config: ResolvedFnosShareBypassConfig,
  ): Promise<ShareValidationFetchResult> {
    const cleanPath = `/s/${shareId}`;
    const fallback: ShareValidationCacheRecord = {
      version: 1,
      valid: false,
      validationState: "unknown",
      shareId,
      cleanPath,
      token: null,
      name: null,
      type: null,
      checkedAt: new Date().toISOString(),
    };

    if (!config.upstreamBaseUrl) {
      return { cacheable: false, data: fallback };
    }

    const controller = new AbortController();
    const timer = setTimeout(
      () => controller.abort(),
      config.policy.upstream_timeout_ms,
    );

    try {
      const targetUrl = new URL(cleanPath, config.upstreamBaseUrl);
      const response = await fetch(targetUrl, {
        method: "GET",
        signal: controller.signal,
        headers: {
          Accept: "text/html,application/xhtml+xml",
        },
      });
      const html = await response.text();
      const parsed = this.parseShareData(html, shareId);
      if (!parsed) {
        return { cacheable: false, data: fallback };
      }

      return {
        cacheable: true,
        data: parsed,
      };
    } catch (error) {
      console.error("[fnos-share] validation failed:", { error, shareId });
      return { cacheable: false, data: fallback };
    } finally {
      clearTimeout(timer);
    }
  }

  private parseShareData(
    html: string,
    shareId: string,
  ): ShareValidationCacheRecord | null {
    const match = html.match(SHARE_SCRIPT_REGEX);
    if (!match?.[2]) return null;

    try {
      const parsed = JSON.parse(match[2]);
      const token =
        typeof parsed?.data?.token === "string" && parsed.data.token.trim()
          ? parsed.data.token.trim()
          : null;
      const name =
        typeof parsed?.data?.name === "string" && parsed.data.name.trim()
          ? parsed.data.name.trim()
          : null;
      const type =
        typeof parsed?.data?.type === "number" ? parsed.data.type : null;
      return normalizeShareValidation({
        version: 1,
        valid: parsed?.code === 0 && !!token,
        validationState: parsed?.code === 0 && !!token ? "valid" : "invalid",
        shareId,
        cleanPath: `/s/${shareId}`,
        token,
        name,
        type,
        checkedAt: new Date().toISOString(),
      });
    } catch {
      return null;
    }
  }

  private async cacheValidation(
    key: string,
    value: ShareValidationCacheRecord,
    ttlSeconds: number,
  ): Promise<void> {
    await redis.set(key, JSON.stringify(value), "EX", ttlSeconds);
  }

  private async getCachedValidation(
    key: string,
  ): Promise<ShareValidationCacheRecord | null> {
    const raw = await redis.get(key);
    if (!raw) return null;
    try {
      return normalizeShareValidation(
        JSON.parse(raw) as ShareValidationCacheRecord,
      );
    } catch {
      return null;
    }
  }

  private async waitForCachedValidation(
    key: string,
    timeoutMs: number,
  ): Promise<ShareValidationCacheRecord | null> {
    const deadline = Date.now() + Math.max(100, timeoutMs);
    while (Date.now() < deadline) {
      const cached = await this.getCachedValidation(key);
      if (cached) return cached;
      await sleep(80);
    }
    return null;
  }

  private async acquireLock(
    key: string,
    ttlSeconds: number,
  ): Promise<string | null> {
    const token = randomBytes(12).toString("hex");
    const result = await redis.set(key, token, "EX", ttlSeconds, "NX");
    return result === "OK" ? token : null;
  }

  private async releaseLock(key: string, token: string): Promise<void> {
    await redis.eval(
      "if redis.call('get', KEYS[1]) == ARGV[1] then return redis.call('del', KEYS[1]) else return 0 end",
      1,
      key,
      token,
    );
  }

  private async getShareSession(
    sessionId: string,
  ): Promise<ShareSessionRecord | null> {
    const raw = await redis.get(toSessionKey(sessionId));
    if (!raw) return null;

    try {
      const parsed = JSON.parse(raw) as ShareSessionRecord;
      return {
        version: 1,
        shareId: parsed.shareId,
        cleanPath: parsed.cleanPath,
        token:
          typeof parsed.token === "string" && parsed.token.trim()
            ? parsed.token
            : null,
        name:
          typeof parsed.name === "string" && parsed.name.trim()
            ? parsed.name
            : null,
        type: typeof parsed.type === "number" ? parsed.type : null,
        issuedAt: parsed.issuedAt,
        lastSeenAt: parsed.lastSeenAt,
      };
    } catch {
      return null;
    }
  }

  private async saveShareSession(
    sessionId: string,
    session: ShareSessionRecord,
    ttlSeconds: number,
  ): Promise<void> {
    const nextSession: ShareSessionRecord = {
      ...session,
      lastSeenAt: new Date().toISOString(),
    };
    await redis.set(
      toSessionKey(sessionId),
      JSON.stringify(nextSession),
      "EX",
      ttlSeconds,
    );
  }
}

export const fnosShareBypassService = new FnosShareBypassService();
