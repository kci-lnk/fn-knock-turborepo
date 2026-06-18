import type { IncomingMessage } from "node:http";
import { BlockList, isIP } from "node:net";
import { randomBytes, scrypt as scryptCallback } from "node:crypto";
import { normalizeCidrLines } from "../../../../packages/admin-shared/src/utils/cidr";
import type { LocaleConfig } from "../../../../packages/i18n/src";
import { configManager, redis } from "./redis";
import { getRequiredEnv } from "./env";
import { getClientIp } from "./auth-request";
import { normalizeIp, isWhitelistExemptIp } from "./ip-normalize";
import { safeEqualString } from "./security";
import { tDefault } from "./i18n";
import { getRuntimeProfile, type DeploymentTarget } from "./runtime-profile";
import { isDockerAdminPanelReauthSessionAllowed } from "./totp-access-scopes";

const DOCKER_ADMIN_PASSWORD_KEY = "fn_knock:docker_admin:password:v1";
const DOCKER_ADMIN_SESSION_PREFIX = "fn_knock:docker_admin:session:v1";
const DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX =
  "fn_knock:docker_admin:login_backoff:v1";
const DEFAULT_SESSION_TTL_SECONDS = 12 * 60 * 60;
const SESSION_TTL_SECONDS = (() => {
  const parsed = Number.parseInt(
    process.env.DOCKER_ADMIN_SESSION_TTL_SECONDS || "",
    10,
  );
  if (!Number.isFinite(parsed)) {
    return DEFAULT_SESSION_TTL_SECONDS;
  }
  return Math.min(7 * 24 * 60 * 60, Math.max(15 * 60, parsed));
})();
const PANEL_REMEMBER_ME_TTL_SECONDS = 30 * 24 * 60 * 60;
const DOCKER_ADMIN_LOGIN_BACKOFF_TTL_SECONDS = 60 * 60;
const DOCKER_ADMIN_LOGIN_BACKOFF_BASE_DELAY_MS = 2000;
const DOCKER_ADMIN_LOGIN_BACKOFF_MAX_DELAY_MS = 15 * 60 * 1000;

const SCRYPT_PARAMS = {
  N: 16384,
  r: 8,
  p: 1,
  keyLength: 64,
  maxmem: 128 * 1024 * 1024,
};
const dockerAdminPanelT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.dockerAdminPanel.${key}`, params);

const scryptAsync = (
  password: string,
  salt: Buffer,
  keyLength: number,
  options: {
    N: number;
    r: number;
    p: number;
    maxmem: number;
  },
) =>
  new Promise<Buffer>((resolve, reject) => {
    scryptCallback(password, salt, keyLength, options, (error, derivedKey) => {
      if (error) {
        reject(error);
        return;
      }
      resolve(derivedKey);
    });
  });

const PROXY_PROTO_HEADERS = [
  "eo-connecting-ip",
  "ali-real-client-ip",
  "x-forwarded-for",
  "x-real-ip",
] as const;

const normalizeTrustedProxyEntry = (value: string): string => {
  const raw = String(value ?? "").trim();
  if (!raw) return "";

  if (!raw.includes("/")) {
    const normalizedIp = normalizeIp(raw);
    if (!normalizedIp) return "";
    return `${normalizedIp}/${isIP(normalizedIp) === 6 ? "128" : "32"}`;
  }

  const [rawAddress = "", rawPrefix = ""] = raw.split("/", 2);
  const normalizedIp = normalizeIp(rawAddress);
  if (!normalizedIp || !/^\d+$/.test(rawPrefix?.trim() || "")) {
    return "";
  }

  const prefix = Number.parseInt(rawPrefix.trim(), 10);
  if (!Number.isFinite(prefix) || prefix < 0) {
    return "";
  }

  const family = isIP(normalizedIp);
  if (family === 4 && prefix > 32) return "";
  if (family === 6 && prefix > 128) return "";
  if (family === 0) return "";

  return `${normalizedIp}/${prefix}`;
};

const parseTrustedProxyCidrs = (value: string | null | undefined): string[] => {
  const tokens = String(value ?? "")
    .split(/[\s,]+/u)
    .map((item) => normalizeTrustedProxyEntry(item))
    .filter(Boolean);

  return normalizeCidrLines(tokens);
};

const TRUSTED_DOCKER_ADMIN_PROXY_CIDRS = parseTrustedProxyCidrs(
  process.env.DOCKER_ADMIN_TRUSTED_PROXY_CIDRS,
);

const trustedDockerAdminProxyBlockList = (() => {
  const blockList = new BlockList();

  for (const cidr of TRUSTED_DOCKER_ADMIN_PROXY_CIDRS) {
    const [rawAddress, rawPrefix] = cidr.split("/", 2);
    const normalizedIp = normalizeIp(rawAddress);
    const prefix = Number.parseInt(rawPrefix || "", 10);
    const family = isIP(normalizedIp);
    if (!normalizedIp || !Number.isFinite(prefix) || family === 0) {
      continue;
    }

    blockList.addSubnet(normalizedIp, prefix, family === 6 ? "ipv6" : "ipv4");
  }

  return blockList;
})();

export const DOCKER_ADMIN_SESSION_COOKIE_NAME = "fn-knock-admin-panel-session";
export const DOCKER_ADMIN_PROXY_HEADER_NAME = "x-fn-knock-admin-proxy";
export const DOCKER_ADMIN_DISCOVER_IP_HEADER_NAME =
  "x-fn-knock-docker-discover-ip";
export const UPSTREAM_PRIVATE_IPV4_HEADER_NAME =
  "x-reauth-upstream-private-ipv4";

const getDockerAdminProxySecretValue = () =>
  getRequiredEnv("ADMIN_PROXY_SECRET");

export interface DockerAdminPasswordRecord {
  algorithm: "scrypt";
  salt: string;
  hash: string;
  n: number;
  r: number;
  p: number;
  key_length: number;
  created_at: string;
  updated_at: string;
}

export interface DockerAdminSessionRecord {
  id: string;
  created_at: string;
  updated_at: string;
  expires_at: string;
  ttl_seconds: number;
  ip: string;
  user_agent: string;
}

export type DockerAdminAuthSource = "panel_session" | "reauth_session";

export interface DockerAdminAuthContext {
  authenticated: boolean;
  auth_source: DockerAdminAuthSource | null;
  session_expires_at: string | null;
}

export interface DockerAdminBootstrapState {
  deployment_target: DeploymentTarget;
  enabled: boolean;
  password_configured: boolean;
  authenticated: boolean;
  auth_source: DockerAdminAuthSource | null;
  session_expires_at: string | null;
  locale: LocaleConfig;
}

export interface DockerAdminResetSummary {
  password_cleared: boolean;
  sessions_cleared: number;
  login_failures_cleared: number;
}

interface DockerAdminLoginAttemptRecord {
  ip: string;
  attempts: number;
  last_attempt_at: string;
  blocked_until: number;
}

const toIso = (value = Date.now()) => new Date(value).toISOString();

const sessionKey = (sessionId: string) =>
  `${DOCKER_ADMIN_SESSION_PREFIX}:${sessionId}`;

const loginBackoffKey = (ip: string) =>
  `${DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX}:${ip}`;

const scanKeysByPattern = async (pattern: string): Promise<string[]> => {
  const keys: string[] = [];
  let cursor = "0";

  do {
    const [nextCursor, batch] = await redis.scan(
      cursor,
      "MATCH",
      pattern,
      "COUNT",
      200,
    );
    cursor = nextCursor;
    if (batch.length > 0) {
      keys.push(...batch);
    }
  } while (cursor !== "0");

  return keys;
};

const deleteKeys = async (keys: string[]): Promise<number> => {
  if (keys.length === 0) return 0;

  let deleted = 0;
  for (let index = 0; index < keys.length; index += 200) {
    const batch = keys.slice(index, index + 200);
    deleted += await redis.del(...batch);
  }

  return deleted;
};

const clearKeysByPattern = async (pattern: string): Promise<number> => {
  const keys = await scanKeysByPattern(pattern);
  return deleteKeys(keys);
};

const parseCookieValue = (
  cookieHeader: string | null | undefined,
  name: string,
): string | null => {
  const cookieSource = String(cookieHeader ?? "");
  if (!cookieSource) return null;

  const escapedName = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = cookieSource.match(
    new RegExp(`(?:^|;\\s*)${escapedName}=([^;]+)`),
  );
  if (!match?.[1]) return null;

  try {
    return decodeURIComponent(match[1]);
  } catch {
    return match[1];
  }
};

const normalizeIncomingForwardedIp = (
  headerValue: string | string[] | null | undefined,
): string => {
  const raw = String(
    Array.isArray(headerValue) ? headerValue[0] : (headerValue ?? ""),
  ).trim();
  if (!raw) return "";

  return normalizeIp(raw.split(",")[0]?.trim());
};

export const resolveClientIpFromIncomingMessage = (
  request: IncomingMessage,
): string => {
  const socketIp = normalizeIp(request.socket.remoteAddress);
  const forwardedIp = PROXY_PROTO_HEADERS.map((headerName) =>
    normalizeIncomingForwardedIp(request.headers[headerName]),
  ).find(Boolean);

  if (socketIp && isWhitelistExemptIp(socketIp) && forwardedIp) {
    return forwardedIp;
  }

  return socketIp || forwardedIp || "";
};

const isTrustedDockerAdminIngressIp = (
  ip: string | null | undefined,
): boolean => {
  const normalizedIp = normalizeIp(ip);
  if (!normalizedIp) return false;
  if (isWhitelistExemptIp(normalizedIp)) return true;

  const family = isIP(normalizedIp);
  if (family === 4) {
    return trustedDockerAdminProxyBlockList.check(normalizedIp, "ipv4");
  }
  if (family === 6) {
    return trustedDockerAdminProxyBlockList.check(normalizedIp, "ipv6");
  }
  return false;
};

export interface DockerAdminIncomingRequestContext {
  socketIp: string;
  forwardedIp: string;
  clientIp: string;
  trustedIngress: boolean;
  viaForwardedHeaders: boolean;
}

export const resolveDockerAdminIncomingRequestContext = (
  request: IncomingMessage,
): DockerAdminIncomingRequestContext => {
  const socketIp = normalizeIp(request.socket.remoteAddress);
  const forwardedIp = PROXY_PROTO_HEADERS.map((headerName) =>
    normalizeIncomingForwardedIp(request.headers[headerName]),
  ).find(Boolean);
  const trustedIngress = isTrustedDockerAdminIngressIp(socketIp);
  const viaForwardedHeaders = Boolean(trustedIngress && forwardedIp);
  const clientIp =
    (viaForwardedHeaders ? forwardedIp : socketIp) || forwardedIp || "";

  return {
    socketIp,
    forwardedIp: forwardedIp || "",
    clientIp,
    trustedIngress,
    viaForwardedHeaders,
  };
};

export const isPrivateNetworkClient = (ip: string | null | undefined) =>
  isWhitelistExemptIp(ip);

export const getDockerAdminTrustedProxyCidrs = (): string[] => [
  ...TRUSTED_DOCKER_ADMIN_PROXY_CIDRS,
];

export const resolveDockerAdminDiscoverIpFromIncomingMessage = (
  request: IncomingMessage,
  context: DockerAdminIncomingRequestContext,
): string => {
  if (!context.trustedIngress || !context.viaForwardedHeaders) {
    return "";
  }

  const headerValue = request.headers[UPSTREAM_PRIVATE_IPV4_HEADER_NAME];
  const raw = Array.isArray(headerValue) ? headerValue[0] : headerValue;
  const normalized = normalizeIp(
    String(raw ?? "")
      .split(",")[0]
      ?.trim(),
  );
  if (!normalized || isIP(normalized) !== 4) {
    return "";
  }

  return isWhitelistExemptIp(normalized) ? normalized : "";
};

const normalizeDockerAdminTrackingIp = (ip: string | null | undefined) => {
  const normalized = normalizeIp(ip);
  if (normalized) return normalized;

  const raw = String(ip ?? "").trim();
  return raw || "unknown";
};

const normalizeDockerAdminUserAgent = (
  userAgent: string | null | undefined,
) => {
  const normalized = String(userAgent ?? "")
    .trim()
    .slice(0, 512);
  return normalized || "unknown";
};

const derivePasswordHash = async (
  password: string,
  saltHex: string,
): Promise<string> => {
  const derived = (await scryptAsync(
    password,
    Buffer.from(saltHex, "hex"),
    SCRYPT_PARAMS.keyLength,
    {
      N: SCRYPT_PARAMS.N,
      r: SCRYPT_PARAMS.r,
      p: SCRYPT_PARAMS.p,
      maxmem: SCRYPT_PARAMS.maxmem,
    },
  )) as Buffer;

  return derived.toString("hex");
};

export const validateDockerAdminPassword = (
  password: string,
): string | null => {
  if (password.length < 6) {
    return dockerAdminPanelT("passwordTooShort");
  }
  if (password.length > 128) {
    return dockerAdminPanelT("passwordTooLong");
  }
  if (/\s/.test(password)) {
    return dockerAdminPanelT("passwordWhitespace");
  }
  if (!/[A-Za-z]/.test(password) || !/\d/.test(password)) {
    return dockerAdminPanelT("passwordNeedsLettersAndNumbers");
  }

  return null;
};

const getPasswordRecord =
  async (): Promise<DockerAdminPasswordRecord | null> => {
    const raw = await redis.get(DOCKER_ADMIN_PASSWORD_KEY);
    if (!raw) return null;

    try {
      const parsed = JSON.parse(raw) as Partial<DockerAdminPasswordRecord>;
      if (
        parsed.algorithm !== "scrypt" ||
        typeof parsed.salt !== "string" ||
        typeof parsed.hash !== "string" ||
        typeof parsed.created_at !== "string" ||
        typeof parsed.updated_at !== "string"
      ) {
        return null;
      }

      return {
        algorithm: "scrypt",
        salt: parsed.salt,
        hash: parsed.hash,
        n: parsed.n || SCRYPT_PARAMS.N,
        r: parsed.r || SCRYPT_PARAMS.r,
        p: parsed.p || SCRYPT_PARAMS.p,
        key_length: parsed.key_length || SCRYPT_PARAMS.keyLength,
        created_at: parsed.created_at,
        updated_at: parsed.updated_at,
      };
    } catch {
      return null;
    }
  };

const getSessionRecord = async (
  sessionId: string,
): Promise<DockerAdminSessionRecord | null> => {
  const raw = await redis.get(sessionKey(sessionId));
  if (!raw) return null;

  try {
    const parsed = JSON.parse(raw) as Partial<DockerAdminSessionRecord>;
    if (
      typeof parsed.id !== "string" ||
      typeof parsed.created_at !== "string" ||
      typeof parsed.updated_at !== "string" ||
      typeof parsed.expires_at !== "string" ||
      typeof parsed.ip !== "string" ||
      typeof parsed.user_agent !== "string"
    ) {
      return null;
    }

    const ttlSeconds =
      typeof parsed.ttl_seconds === "number" &&
      Number.isFinite(parsed.ttl_seconds) &&
      parsed.ttl_seconds > 0
        ? Math.max(1, Math.trunc(parsed.ttl_seconds))
        : SESSION_TTL_SECONDS;

    return {
      id: parsed.id,
      created_at: parsed.created_at,
      updated_at: parsed.updated_at,
      expires_at: parsed.expires_at,
      ttl_seconds: ttlSeconds,
      ip: parsed.ip,
      user_agent: parsed.user_agent,
    };
  } catch {
    return null;
  }
};

const persistSessionRecord = async (
  record: DockerAdminSessionRecord,
): Promise<void> => {
  const expiresAt = Date.parse(record.expires_at);
  const ttlSeconds = Math.max(1, Math.ceil((expiresAt - Date.now()) / 1000));

  await redis.set(
    sessionKey(record.id),
    JSON.stringify(record),
    "EX",
    ttlSeconds,
  );
};

const isSecureRequest = (request: Request): boolean => {
  const forwardedProto = request.headers
    .get("x-forwarded-proto")
    ?.split(",")[0]
    ?.trim()
    ?.toLowerCase();

  if (forwardedProto === "https") return true;
  if (forwardedProto === "http") return false;

  try {
    return new URL(request.url).protocol === "https:";
  } catch {
    return false;
  }
};

export const isDockerAdminProxyRequest = (request: Request): boolean => {
  const headerValue = request.headers
    .get(DOCKER_ADMIN_PROXY_HEADER_NAME)
    ?.trim();
  if (!headerValue) return false;

  return safeEqualString(headerValue, getDockerAdminProxySecretValue());
};

export const getDockerAdminProxySecret = () => getDockerAdminProxySecretValue();

export const isDockerAdminPublicPath = (path: string): boolean =>
  path === "/api/admin/healthz" ||
  path === "/api/admin/panel/bootstrap" ||
  path === "/api/admin/panel/login" ||
  path === "/api/admin/panel/password" ||
  path === "/api/admin/panel/logout";

export const isDockerAdminProtectedPath = (path: string): boolean =>
  path.startsWith("/api/admin") ||
  path === "/docs" ||
  path.startsWith("/docs/") ||
  path.startsWith("/swagger-ui");

const getLoginAttemptRecord = async (
  ip: string,
): Promise<DockerAdminLoginAttemptRecord | null> => {
  const raw = await redis.get(loginBackoffKey(ip));
  if (!raw) return null;

  try {
    const parsed = JSON.parse(raw) as Partial<DockerAdminLoginAttemptRecord>;
    if (
      typeof parsed.ip !== "string" ||
      !Number.isFinite(parsed.attempts) ||
      typeof parsed.last_attempt_at !== "string" ||
      !Number.isFinite(parsed.blocked_until)
    ) {
      return null;
    }

    const attempts = Number(parsed.attempts);
    const blockedUntil = Number(parsed.blocked_until);

    return {
      ip: parsed.ip,
      attempts: Math.max(0, Math.trunc(attempts)),
      last_attempt_at: parsed.last_attempt_at,
      blocked_until: Math.trunc(blockedUntil),
    };
  } catch {
    return null;
  }
};

const persistLoginAttemptRecord = async (
  record: DockerAdminLoginAttemptRecord,
): Promise<void> => {
  await redis.set(
    loginBackoffKey(record.ip),
    JSON.stringify(record),
    "EX",
    DOCKER_ADMIN_LOGIN_BACKOFF_TTL_SECONDS,
  );
};

const calculateLoginBackoffMs = (attempts: number): number => {
  const exponent = Math.max(0, Math.trunc(attempts) - 1);
  const delay = DOCKER_ADMIN_LOGIN_BACKOFF_BASE_DELAY_MS * 2 ** exponent;
  return Math.min(DOCKER_ADMIN_LOGIN_BACKOFF_MAX_DELAY_MS, delay);
};

const requestMatchesSessionRecord = (
  record: DockerAdminSessionRecord,
  request: Request,
): boolean => {
  const requestIp = normalizeDockerAdminTrackingIp(getClientIp(request));
  const requestUserAgent = normalizeDockerAdminUserAgent(
    request.headers.get("user-agent"),
  );

  return (
    safeEqualString(record.ip, requestIp) &&
    safeEqualString(record.user_agent, requestUserAgent)
  );
};

const resolveReauthSessionFromRequest = async (
  request: Request,
): Promise<DockerAdminAuthContext | null> => {
  const sessionId = parseCookieValue(
    request.headers.get("cookie"),
    "x-go-reauth-proxy-session-id",
  );
  if (!sessionId) return null;

  try {
    const [session, totpCredentials] = await Promise.all([
      configManager.getSession(sessionId),
      configManager.getTOTPCredentials(),
    ]);
    if (
      !isDockerAdminPanelReauthSessionAllowed({
        session,
        totpCredentials,
      })
    ) {
      return null;
    }

    return {
      authenticated: true,
      auth_source: "reauth_session",
      session_expires_at: session?.expiresAt || null,
    };
  } catch (error) {
    console.warn("[docker-admin] failed to resolve reauth session:", error);
    return null;
  }
};

export const dockerAdminPanelManager = {
  sessionTtlSeconds: SESSION_TTL_SECONDS,
  rememberMeSessionTtlSeconds: PANEL_REMEMBER_ME_TTL_SECONDS,

  async isPasswordConfigured(): Promise<boolean> {
    return Boolean(await getPasswordRecord());
  },

  async verifyPassword(password: string): Promise<boolean> {
    const record = await getPasswordRecord();
    if (!record) return false;

    const derivedHash = await derivePasswordHash(password, record.salt);
    return safeEqualString(derivedHash, record.hash);
  },

  async setPassword(password: string): Promise<void> {
    const passwordValidationMessage = validateDockerAdminPassword(password);
    if (passwordValidationMessage) {
      throw new Error(passwordValidationMessage);
    }

    const existingRecord = await getPasswordRecord();
    if (existingRecord) {
      throw new Error(dockerAdminPanelT("passwordAlreadyConfigured"));
    }

    const now = toIso();
    const salt = randomBytes(16).toString("hex");
    const hash = await derivePasswordHash(password, salt);
    const nextRecord: DockerAdminPasswordRecord = {
      algorithm: "scrypt",
      salt,
      hash,
      n: SCRYPT_PARAMS.N,
      r: SCRYPT_PARAMS.r,
      p: SCRYPT_PARAMS.p,
      key_length: SCRYPT_PARAMS.keyLength,
      created_at: now,
      updated_at: now,
    };

    await redis.set(DOCKER_ADMIN_PASSWORD_KEY, JSON.stringify(nextRecord));
  },

  async changePassword(password: string): Promise<void> {
    const passwordValidationMessage = validateDockerAdminPassword(password);
    if (passwordValidationMessage) {
      throw new Error(passwordValidationMessage);
    }

    const existingRecord = await getPasswordRecord();
    if (!existingRecord) {
      throw new Error(dockerAdminPanelT("passwordNotConfigured"));
    }

    const isSamePassword = await this.verifyPassword(password);
    if (isSamePassword) {
      throw new Error(dockerAdminPanelT("newPasswordSameAsCurrent"));
    }

    const now = toIso();
    const salt = randomBytes(16).toString("hex");
    const hash = await derivePasswordHash(password, salt);
    const nextRecord: DockerAdminPasswordRecord = {
      algorithm: "scrypt",
      salt,
      hash,
      n: SCRYPT_PARAMS.N,
      r: SCRYPT_PARAMS.r,
      p: SCRYPT_PARAMS.p,
      key_length: SCRYPT_PARAMS.keyLength,
      created_at: existingRecord.created_at,
      updated_at: now,
    };

    await redis.set(DOCKER_ADMIN_PASSWORD_KEY, JSON.stringify(nextRecord));
    await Promise.all([this.clearAllSessions(), this.clearAllLoginFailures()]);
  },

  async createSession(args: {
    ip: string;
    userAgent: string;
    ttlSeconds?: number;
  }): Promise<DockerAdminSessionRecord> {
    const now = Date.now();
    const sessionId = randomBytes(32).toString("hex");
    const ttlSeconds =
      typeof args.ttlSeconds === "number" &&
      Number.isFinite(args.ttlSeconds) &&
      args.ttlSeconds > 0
        ? Math.max(1, Math.trunc(args.ttlSeconds))
        : SESSION_TTL_SECONDS;
    const record: DockerAdminSessionRecord = {
      id: sessionId,
      created_at: toIso(now),
      updated_at: toIso(now),
      expires_at: toIso(now + ttlSeconds * 1000),
      ttl_seconds: ttlSeconds,
      ip: normalizeDockerAdminTrackingIp(args.ip),
      user_agent: normalizeDockerAdminUserAgent(args.userAgent),
    };

    await persistSessionRecord(record);
    return record;
  },

  async deleteSessionById(sessionId: string | null | undefined): Promise<void> {
    if (!sessionId) return;
    await redis.del(sessionKey(sessionId));
  },

  async deleteSessionFromRequest(request: Request): Promise<void> {
    const sessionId = parseCookieValue(
      request.headers.get("cookie"),
      DOCKER_ADMIN_SESSION_COOKIE_NAME,
    );

    await this.deleteSessionById(sessionId);
  },

  async resolveSessionFromRequest(
    request: Request,
  ): Promise<DockerAdminSessionRecord | null> {
    const sessionId = parseCookieValue(
      request.headers.get("cookie"),
      DOCKER_ADMIN_SESSION_COOKIE_NAME,
    );
    if (!sessionId) return null;

    const record = await getSessionRecord(sessionId);
    if (!record) return null;

    const expiresAt = Date.parse(record.expires_at);
    if (!Number.isFinite(expiresAt) || expiresAt <= Date.now()) {
      await this.deleteSessionById(sessionId);
      return null;
    }

    if (!requestMatchesSessionRecord(record, request)) {
      await this.deleteSessionById(sessionId);
      return null;
    }

    const refreshedRecord: DockerAdminSessionRecord = {
      ...record,
      updated_at: toIso(),
      expires_at: toIso(Date.now() + record.ttl_seconds * 1000),
    };
    await persistSessionRecord(refreshedRecord);

    return refreshedRecord;
  },

  async resolveAuthContextFromRequest(
    request: Request,
  ): Promise<DockerAdminAuthContext> {
    const panelSession = await this.resolveSessionFromRequest(request);
    if (panelSession) {
      return {
        authenticated: true,
        auth_source: "panel_session",
        session_expires_at: panelSession.expires_at,
      };
    }

    const reauthSession = await resolveReauthSessionFromRequest(request);
    if (reauthSession) {
      return reauthSession;
    }

    return {
      authenticated: false,
      auth_source: null,
      session_expires_at: null,
    };
  },

  async buildBootstrapState(
    request: Request,
    enabled: boolean,
    locale: LocaleConfig,
  ): Promise<DockerAdminBootstrapState> {
    const deploymentTarget = getRuntimeProfile().deployment_target;

    if (!enabled) {
      return {
        deployment_target: deploymentTarget,
        enabled: false,
        password_configured: false,
        authenticated: true,
        auth_source: null,
        session_expires_at: null,
        locale,
      };
    }

    const [passwordConfigured, authContext] = await Promise.all([
      this.isPasswordConfigured(),
      this.resolveAuthContextFromRequest(request),
    ]);

    return {
      deployment_target: deploymentTarget,
      enabled: true,
      password_configured: passwordConfigured,
      authenticated: authContext.authenticated,
      auth_source: authContext.auth_source,
      session_expires_at: authContext.session_expires_at,
      locale,
    };
  },

  async ensureLoginAllowed(ip: string | null | undefined): Promise<{
    allowed: boolean;
    retryAfter?: number;
    blockedUntil?: number;
  }> {
    const trackingIp = normalizeDockerAdminTrackingIp(ip);
    const record = await getLoginAttemptRecord(trackingIp);
    if (!record) {
      return { allowed: true };
    }

    const now = Date.now();
    if (!Number.isFinite(record.blocked_until) || record.blocked_until <= now) {
      return { allowed: true };
    }

    return {
      allowed: false,
      retryAfter: Math.max(1, Math.ceil((record.blocked_until - now) / 1000)),
      blockedUntil: record.blocked_until,
    };
  },

  async registerLoginFailure(ip: string | null | undefined): Promise<{
    attempts: number;
    retryAfter: number;
    blockedUntil: number;
  }> {
    const trackingIp = normalizeDockerAdminTrackingIp(ip);
    const previousRecord = await getLoginAttemptRecord(trackingIp);
    const attempts = (previousRecord?.attempts ?? 0) + 1;
    const backoffMs = calculateLoginBackoffMs(attempts);
    const blockedUntil = Date.now() + backoffMs;

    await persistLoginAttemptRecord({
      ip: trackingIp,
      attempts,
      last_attempt_at: toIso(),
      blocked_until: blockedUntil,
    });

    return {
      attempts,
      retryAfter: Math.max(1, Math.ceil(backoffMs / 1000)),
      blockedUntil,
    };
  },

  async resetLoginFailures(ip: string | null | undefined): Promise<void> {
    const trackingIp = normalizeDockerAdminTrackingIp(ip);
    await redis.del(loginBackoffKey(trackingIp));
  },

  async clearAllSessions(): Promise<number> {
    return clearKeysByPattern(`${DOCKER_ADMIN_SESSION_PREFIX}:*`);
  },

  async clearAllLoginFailures(): Promise<number> {
    return clearKeysByPattern(`${DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX}:*`);
  },

  async resetPasswordState(): Promise<DockerAdminResetSummary> {
    const [passwordDeleted, sessionsCleared, loginFailuresCleared] =
      await Promise.all([
        redis.del(DOCKER_ADMIN_PASSWORD_KEY),
        this.clearAllSessions(),
        this.clearAllLoginFailures(),
      ]);

    return {
      password_cleared: passwordDeleted > 0,
      sessions_cleared: sessionsCleared,
      login_failures_cleared: loginFailuresCleared,
    };
  },

  isSecureRequest,
};
