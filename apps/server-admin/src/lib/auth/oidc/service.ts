import { createHash, randomBytes } from "node:crypto";
import { createRemoteJWKSet, jwtVerify, type JWTPayload } from "jose";
import { configManager, type AppConfig } from "../../redis";
import { getBooleanEnv } from "../../env";
import { safeEqualString } from "../../security";
import {
  resolvePublicAuthBaseUrl,
  resolveSafeRedirectUri,
} from "../../subdomain-mode";
import { tDefault } from "../../i18n";
import {
  getDefaultConnectionConfig,
  getOIDCProviderDefinition,
  isExternalAuthProviderType,
  OIDC_PROVIDER_CATALOG,
} from "./catalog";
import { oidcRedisStore } from "./redis-store";
import type {
  ExternalAuthProfile,
  ExternalAuthProviderType,
  OIDCAuthState,
  OIDCAuthStateMode,
  OIDCBinding,
  OIDCBindInvite,
  OIDCDiscoveryDocument,
  OIDCProvider,
  OIDCProviderConnectionConfig,
  OIDCProviderUpdateInput,
  OIDCProviderUpsertInput,
  OIDCProviderView,
} from "./types";

const STATE_TTL_SECONDS = 10 * 60;
export const OIDC_STATE_TTL_SECONDS = STATE_TTL_SECONDS;
const DEFAULT_INVITE_TTL_SECONDS = 30 * 60;
const MAX_INVITE_TTL_SECONDS = 7 * 24 * 60 * 60;
const LOGIN_ERROR_TTL_SECONDS = 5 * 60;
const LOGIN_ERROR_MESSAGE_MAX_LENGTH = 240;
const REQUEST_TIMEOUT_MS = 7000;
const oidcT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
): string => tDefault(`server.oidc.${key}`, params);
export const OIDC_CALLBACK_STATE_EXPIRED_MESSAGE =
  oidcT("callbackStateExpired");
const TRUST_OIDC_FORWARDED_HEADERS =
  getBooleanEnv("OIDC_TRUST_FORWARDED_HEADERS", false) ||
  getBooleanEnv("AUTH_TRUST_FORWARDED_HEADERS", false);
const RESERVED_EXTRA_AUTH_PARAM_KEYS = new Set([
  "client_id",
  "client_secret",
  "response_type",
  "redirect_uri",
  "scope",
  "state",
  "nonce",
  "code_challenge",
  "code_challenge_method",
  "code_verifier",
  "grant_type",
  "code",
]);
const remoteJwksCache = new Map<
  string,
  ReturnType<typeof createRemoteJWKSet>
>();

const nowIso = () => new Date().toISOString();

const createId = (prefix: string) =>
  `${prefix}_${randomBytes(10).toString("hex")}`;

const createPublicToken = () => randomBytes(32).toString("base64url");

export const hashOIDCToken = (value: string) =>
  createHash("sha256").update(value).digest("hex");

const base64Url = (value: Buffer) =>
  value
    .toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "");

const createPkceVerifier = () => base64Url(randomBytes(32));

const createPkceChallenge = (verifier: string) =>
  base64Url(createHash("sha256").update(verifier).digest());

const normalizeString = (value: unknown) =>
  typeof value === "string" ? value.trim() : "";

const normalizeOptionalString = (value: unknown) => {
  const normalized = normalizeString(value);
  return normalized || undefined;
};

const normalizeLoginErrorMessage = (value: unknown) => {
  const normalized = normalizeString(value) || oidcT("loginFailedRetry");
  return normalized.length > LOGIN_ERROR_MESSAGE_MAX_LENGTH
    ? `${normalized.slice(0, LOGIN_ERROR_MESSAGE_MAX_LENGTH)}...`
    : normalized;
};

const normalizeScopes = (value: unknown, fallback: string[]) => {
  if (Array.isArray(value)) {
    const scopes = [
      ...new Set(
        value
          .map((item) => normalizeString(item))
          .filter((item) => item.length > 0),
      ),
    ];
    return scopes.length ? scopes : fallback;
  }
  const raw = normalizeString(value);
  if (!raw) return fallback;
  const scopes = [...new Set(raw.split(/[,\s]+/).filter(Boolean))];
  return scopes.length ? scopes : fallback;
};

const normalizeStringRecord = (value: unknown) => {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }
  const record: Record<string, string> = {};
  for (const [key, entry] of Object.entries(value)) {
    const normalizedKey = key.trim();
    const normalizedValue = normalizeString(entry);
    if (normalizedKey && normalizedValue) {
      record[normalizedKey] = normalizedValue;
    }
  }
  return Object.keys(record).length ? record : undefined;
};

const assertExtraAuthParamKeyAllowed = (key: string) => {
  const normalizedKey = key.trim().toLowerCase();
  if (RESERVED_EXTRA_AUTH_PARAM_KEYS.has(normalizedKey)) {
    throw new Error(oidcT("reservedExtraAuthParam", { key }));
  }
};

const assertExtraAuthParamsAllowed = (extraParams?: Record<string, string>) => {
  for (const key of Object.keys(extraParams || {})) {
    assertExtraAuthParamKeyAllowed(key);
  }
};

const normalizeExtraAuthParams = (value: unknown) => {
  const record = normalizeStringRecord(value);
  if (!record) return undefined;
  assertExtraAuthParamsAllowed(record);
  return record;
};

const applyExtraAuthParams = (
  params: URLSearchParams,
  extraParams?: Record<string, string>,
) => {
  assertExtraAuthParamsAllowed(extraParams);
  for (const [key, value] of Object.entries(extraParams || {})) {
    params.set(key, value);
  }
};

export const isOIDCFlowTokenValid = (
  state: string | null | undefined,
  flowToken: string | null | undefined,
) => {
  const normalizedState = normalizeString(state);
  const normalizedFlowToken = normalizeString(flowToken);
  if (!normalizedState || !normalizedFlowToken) return false;
  return safeEqualString(hashOIDCToken(normalizedState), normalizedFlowToken);
};

const assertOIDCFlowTokenValid = (
  state: string,
  flowToken: string | null | undefined,
) => {
  if (!isOIDCFlowTokenValid(state, flowToken)) {
    throw new Error(OIDC_CALLBACK_STATE_EXPIRED_MESSAGE);
  }
};

const isConnectionValuePresent = (value: unknown) =>
  Array.isArray(value)
    ? value.length > 0
    : typeof value === "string"
      ? value.trim().length > 0
      : value !== undefined && value !== null;

const assertHttpUrl = (value: string, label: string) => {
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    throw new Error(oidcT("urlInvalid", { label }));
  }
  if (parsed.protocol !== "https:" && parsed.hostname !== "localhost") {
    throw new Error(oidcT("urlMustUseHttps", { label }));
  }
};

const normalizeProviderConnectionConfig = (
  type: ExternalAuthProviderType,
  raw: Record<string, unknown> = {},
  options: { allowIncomplete?: boolean } = {},
): OIDCProviderConnectionConfig => {
  const definition = getOIDCProviderDefinition(type);
  if (!definition) throw new Error(oidcT("providerUnsupported"));
  const defaults = getDefaultConnectionConfig(type);
  const tenant = normalizeOptionalString(raw.tenant) || defaults.tenant;
  const issuer =
    normalizeOptionalString(raw.issuer) ||
    (type === "microsoft" && tenant
      ? `https://login.microsoftonline.com/${tenant}/v2.0`
      : defaults.issuer);
  const config: OIDCProviderConnectionConfig = {
    ...defaults,
    client_id: normalizeString(raw.client_id),
    client_secret: normalizeString(raw.client_secret),
    ...(issuer ? { issuer } : {}),
    ...(tenant ? { tenant } : {}),
    authorization_endpoint:
      normalizeOptionalString(raw.authorization_endpoint) ||
      defaults.authorization_endpoint,
    token_endpoint:
      normalizeOptionalString(raw.token_endpoint) || defaults.token_endpoint,
    userinfo_endpoint:
      normalizeOptionalString(raw.userinfo_endpoint) ||
      defaults.userinfo_endpoint,
    jwks_uri: normalizeOptionalString(raw.jwks_uri) || defaults.jwks_uri,
    emails_endpoint:
      normalizeOptionalString(raw.emails_endpoint) || defaults.emails_endpoint,
    scopes: normalizeScopes(raw.scopes, definition.default_scopes),
    extra_auth_params: normalizeExtraAuthParams(raw.extra_auth_params),
  };

  const missingFields = definition.required_fields.filter(
    (field) => !isConnectionValuePresent(config[field]),
  );
  if (missingFields.length && !options.allowIncomplete) {
    throw new Error(
      oidcT("providerMissingRequiredConfig", {
        provider: definition.label,
        fields: missingFields.join(", "),
      }),
    );
  }

  for (const field of [
    "issuer",
    "authorization_endpoint",
    "token_endpoint",
    "userinfo_endpoint",
    "jwks_uri",
    "emails_endpoint",
  ] as const) {
    const value = config[field];
    if (typeof value === "string" && value.trim()) {
      assertHttpUrl(value.trim(), field);
    }
  }

  return config;
};

const getMissingProviderRequiredFields = (provider: OIDCProvider) => {
  const definition = getOIDCProviderDefinition(provider.type);
  if (!definition) return ["type"];
  return definition.required_fields.filter(
    (field) => !isConnectionValuePresent(provider.connection_config[field]),
  );
};

const assertProviderReady = (provider: OIDCProvider) => {
  const missingFields = getMissingProviderRequiredFields(provider);
  if (missingFields.length) {
    throw new Error(
      oidcT("providerMissingRequiredFields", {
        fields: missingFields.join(", "),
      }),
    );
  }
  assertExtraAuthParamsAllowed(provider.connection_config.extra_auth_params);
};

const maskSensitiveValue = (value: unknown) => {
  if (value === undefined || value === null || value === "") return "";
  if (typeof value !== "string") return "[configured]";
  return value.length <= 8 ? "********" : `${value.slice(0, 2)}******`;
};

const maskProvider = (
  provider: OIDCProvider,
  callbackUrl?: string,
): OIDCProviderView => {
  const masked: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(provider.connection_config)) {
    masked[key] = key === "client_secret" ? maskSensitiveValue(value) : value;
  }
  return {
    id: provider.id,
    type: provider.type,
    protocol: provider.protocol,
    name: provider.name,
    enabled: provider.enabled,
    created_at: provider.created_at,
    updated_at: provider.updated_at,
    last_test_at: provider.last_test_at,
    last_test_status: provider.last_test_status,
    last_error: provider.last_error,
    connection_config_masked: masked,
    ...(callbackUrl ? { callback_url: callbackUrl } : {}),
  };
};

const withTimeout = (timeoutMs = REQUEST_TIMEOUT_MS) => {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  return { signal: controller.signal, done: () => clearTimeout(timer) };
};

const fetchText = async (url: string, init?: RequestInit) => {
  const timeout = withTimeout();
  try {
    const response = await fetch(url, {
      ...init,
      signal: timeout.signal,
    });
    const text = await response.text();
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${text.slice(0, 160)}`);
    }
    return {
      response,
      text,
      contentType: response.headers.get("content-type") || "",
    };
  } finally {
    timeout.done();
  }
};

const parseJsonOrForm = (text: string, contentType = "") => {
  const trimmed = text.trim();
  if (contentType.includes("json") || trimmed.startsWith("{")) {
    return JSON.parse(trimmed) as Record<string, unknown>;
  }
  return Object.fromEntries(new URLSearchParams(trimmed).entries());
};

const parseAccessToken = (payload: Record<string, unknown>) => {
  const accessToken = normalizeString(payload.access_token);
  if (!accessToken) {
    const message =
      normalizeString(payload.error_description) ||
      normalizeString(payload.error) ||
      oidcT("accessTokenMissing");
    throw new Error(message);
  }
  return accessToken;
};

const parseIdToken = (payload: Record<string, unknown>) => {
  const idToken = normalizeString(payload.id_token);
  if (!idToken) throw new Error(oidcT("idTokenMissing"));
  return idToken;
};

const getRemoteJwks = (jwksUri: string) => {
  const cached = remoteJwksCache.get(jwksUri);
  if (cached) return cached;
  const jwks = createRemoteJWKSet(new URL(jwksUri));
  remoteJwksCache.set(jwksUri, jwks);
  return jwks;
};

const takeFirstHeaderValue = (value: string | null) => {
  const first = value?.split(",")[0]?.trim();
  return first || "";
};

const normalizeOriginProto = (value: string) =>
  value.trim().replace(/:$/, "").toLowerCase();

const isSafeOriginHost = (host: string) =>
  Boolean(host) && !/[\s,/?#\\@]/.test(host);

const resolveRequestOrigin = (request: Request) => {
  const url = new URL(request.url);
  const requestProto = normalizeOriginProto(url.protocol) || "http";
  const trustedProto = TRUST_OIDC_FORWARDED_HEADERS
    ? takeFirstHeaderValue(request.headers.get("x-forwarded-proto"))
    : "";
  const proto = normalizeOriginProto(trustedProto || requestProto);
  const trustedHost = TRUST_OIDC_FORWARDED_HEADERS
    ? takeFirstHeaderValue(request.headers.get("x-forwarded-host"))
    : "";
  const directHost = request.headers.get("host")?.trim() || url.host;
  const host = trustedHost || directHost;

  if ((proto !== "http" && proto !== "https") || !isSafeOriginHost(host)) {
    throw new Error(oidcT("callbackUrlBuildFailed"));
  }

  return `${proto}://${host}`;
};

const resolveAuthPrefix = (request: Request) => {
  const pathname = new URL(request.url).pathname;
  if (pathname.startsWith("/__auth__/api/auth/")) return "/__auth__";
  if (pathname.startsWith("/auth/api/auth/")) return "/auth";
  return "";
};

const trimTrailingSlash = (value: string) => value.replace(/\/+$/, "");

const resolvePublicAuthAppBaseUrl = (baseUrl: string) => {
  const trimmed = trimTrailingSlash(baseUrl);
  if (!trimmed) return "";
  try {
    const parsed = new URL(trimmed);
    parsed.pathname = parsed.pathname.replace(/\/+$/, "") || "/";
    parsed.search = "";
    parsed.hash = "";
    return trimTrailingSlash(parsed.toString());
  } catch {
    return trimmed;
  }
};

const buildCallbackUrl = (
  providerId: string,
  request: Request,
  config: AppConfig,
) => {
  const publicBaseUrl = resolvePublicAuthBaseUrl(config);
  if (publicBaseUrl) {
    return `${resolvePublicAuthAppBaseUrl(publicBaseUrl)}/api/auth/oidc/callback/${encodeURIComponent(providerId)}`;
  }
  const prefix = resolveAuthPrefix(request);
  return `${resolveRequestOrigin(request)}${prefix}/api/auth/oidc/callback/${encodeURIComponent(providerId)}`;
};

const buildInviteUrl = (token: string, request: Request, config: AppConfig) => {
  const publicBaseUrl = resolvePublicAuthBaseUrl(config);
  const base = publicBaseUrl
    ? resolvePublicAuthAppBaseUrl(publicBaseUrl)
    : `${resolveRequestOrigin(request)}${resolveAuthPrefix(request)}`;
  return `${base}/api/auth/oidc/bind?token=${encodeURIComponent(token)}`;
};

const buildSubjectKey = (providerId: string, issuer: string, subject: string) =>
  createHash("sha256")
    .update(`${providerId}\u0000${issuer}\u0000${subject}`)
    .digest("hex");

const buildOidcDiscoveryUrl = (issuer: string) => {
  const normalized = trimTrailingSlash(issuer);
  return `${normalized}/.well-known/openid-configuration`;
};

const getDiscovery = async (
  provider: OIDCProvider,
): Promise<OIDCDiscoveryDocument> => {
  const cfg = provider.connection_config;
  if (
    cfg.authorization_endpoint &&
    cfg.token_endpoint &&
    cfg.jwks_uri &&
    cfg.issuer
  ) {
    return {
      issuer: cfg.issuer,
      authorization_endpoint: cfg.authorization_endpoint,
      token_endpoint: cfg.token_endpoint,
      ...(cfg.userinfo_endpoint
        ? { userinfo_endpoint: cfg.userinfo_endpoint }
        : {}),
      jwks_uri: cfg.jwks_uri,
    };
  }
  if (!cfg.issuer) throw new Error(oidcT("issuerMissing"));
  const { text, contentType } = await fetchText(
    buildOidcDiscoveryUrl(cfg.issuer),
    {
      headers: { Accept: "application/json" },
    },
  );
  const payload = parseJsonOrForm(text, contentType);
  const issuer = normalizeString(payload.issuer);
  const authorizationEndpoint = normalizeString(payload.authorization_endpoint);
  const tokenEndpoint = normalizeString(payload.token_endpoint);
  const jwksUri = normalizeString(payload.jwks_uri);
  if (!issuer || !authorizationEndpoint || !tokenEndpoint || !jwksUri) {
    throw new Error(oidcT("discoveryMissingFields"));
  }
  return {
    issuer,
    authorization_endpoint: authorizationEndpoint,
    token_endpoint: tokenEndpoint,
    userinfo_endpoint: normalizeOptionalString(payload.userinfo_endpoint),
    jwks_uri: jwksUri,
  };
};

const exchangeFormToken = async (
  tokenEndpoint: string,
  body: URLSearchParams,
  headers?: HeadersInit,
) => {
  const { text, contentType } = await fetchText(tokenEndpoint, {
    method: "POST",
    headers: {
      Accept: "application/json",
      "Content-Type": "application/x-www-form-urlencoded",
      ...(headers || {}),
    },
    body,
  });
  return parseJsonOrForm(text, contentType);
};

const verifyStandardOidcProfile = async (
  provider: OIDCProvider,
  tokenPayload: Record<string, unknown>,
  discovery: OIDCDiscoveryDocument,
  expectedNonce?: string,
): Promise<ExternalAuthProfile> => {
  const idToken = parseIdToken(tokenPayload);
  const issuerForVerify = discovery.issuer.includes("{tenantid}")
    ? undefined
    : discovery.issuer;
  const verified = await jwtVerify(idToken, getRemoteJwks(discovery.jwks_uri), {
    audience: provider.connection_config.client_id,
    ...(issuerForVerify ? { issuer: issuerForVerify } : {}),
  });
  const payload = verified.payload as JWTPayload & Record<string, unknown>;
  if (expectedNonce && payload.nonce !== expectedNonce) {
    throw new Error(oidcT("nonceCheckFailed"));
  }
  if (!issuerForVerify) {
    const issuer = normalizeString(payload.iss);
    if (!issuer || !issuer.startsWith("https://login.microsoftonline.com/")) {
      throw new Error(oidcT("issuerCheckFailed"));
    }
  }
  const subject = normalizeString(payload.sub);
  if (!subject) throw new Error(oidcT("subjectEmpty"));

  let userInfo: Record<string, unknown> = {};
  const accessToken = normalizeString(tokenPayload.access_token);
  if (discovery.userinfo_endpoint && accessToken) {
    try {
      const { text, contentType } = await fetchText(
        discovery.userinfo_endpoint,
        {
          headers: {
            Accept: "application/json",
            Authorization: `Bearer ${accessToken}`,
          },
        },
      );
      userInfo = parseJsonOrForm(text, contentType);
    } catch {
      userInfo = {};
    }
  }

  const pick = (key: string) => userInfo[key] ?? payload[key];
  return {
    issuer: normalizeString(payload.iss) || discovery.issuer,
    subject,
    display_name:
      normalizeOptionalString(pick("name")) ||
      normalizeOptionalString(pick("preferred_username")),
    email: normalizeOptionalString(pick("email")),
    email_verified: Boolean(pick("email_verified")),
    avatar_url: normalizeOptionalString(pick("picture")),
  };
};

const fetchGithubProfile = async (
  provider: OIDCProvider,
  accessToken: string,
): Promise<ExternalAuthProfile> => {
  const cfg = provider.connection_config;
  const userEndpoint = cfg.userinfo_endpoint || "https://api.github.com/user";
  const { text } = await fetchText(userEndpoint, {
    headers: {
      Accept: "application/vnd.github+json",
      Authorization: `Bearer ${accessToken}`,
      "X-GitHub-Api-Version": "2022-11-28",
    },
  });
  const user = JSON.parse(text) as Record<string, unknown>;
  const subject =
    normalizeString(user.id) ||
    (typeof user.id === "number" && Number.isFinite(user.id)
      ? String(user.id)
      : "");
  if (!subject) throw new Error(oidcT("githubUserIdEmpty"));

  let email = normalizeOptionalString(user.email);
  let emailVerified = false;
  if (cfg.emails_endpoint) {
    try {
      const emailsRes = await fetchText(cfg.emails_endpoint, {
        headers: {
          Accept: "application/vnd.github+json",
          Authorization: `Bearer ${accessToken}`,
          "X-GitHub-Api-Version": "2022-11-28",
        },
      });
      const emails = JSON.parse(emailsRes.text) as Array<
        Record<string, unknown>
      >;
      const primary = emails.find((item) => item.primary === true) || emails[0];
      if (primary) {
        email = normalizeOptionalString(primary.email) || email;
        emailVerified = primary.verified === true;
      }
    } catch {
      emailVerified = Boolean(email);
    }
  }

  return {
    issuer: "github",
    subject,
    display_name:
      normalizeOptionalString(user.name) || normalizeOptionalString(user.login),
    email,
    email_verified: emailVerified,
    avatar_url: normalizeOptionalString(user.avatar_url),
  };
};

export class OIDCAuthService {
  listProviderCatalog() {
    return OIDC_PROVIDER_CATALOG;
  }

  async listProviders(request?: Request): Promise<OIDCProviderView[]> {
    const [providers, config] = await Promise.all([
      oidcRedisStore.listProviders(),
      request ? configManager.getConfig() : Promise.resolve(null),
    ]);
    return providers.map((provider) =>
      maskProvider(
        provider,
        request && config ? buildCallbackUrl(provider.id, request, config) : "",
      ),
    );
  }

  async listPublicProviders() {
    const providers = await oidcRedisStore.listProviders();
    return providers
      .filter((provider) => provider.enabled)
      .filter(
        (provider) => getMissingProviderRequiredFields(provider).length === 0,
      )
      .map((provider) => ({
        id: provider.id,
        type: provider.type,
        name: provider.name,
        protocol: provider.protocol,
      }));
  }

  async getProvider(id: string) {
    return oidcRedisStore.getProvider(id);
  }

  async createProvider(input: OIDCProviderUpsertInput) {
    if (!isExternalAuthProviderType(input.type)) {
      throw new Error(oidcT("providerUnsupported"));
    }
    const definition = getOIDCProviderDefinition(input.type);
    if (!definition) throw new Error(oidcT("providerUnsupported"));
    const now = nowIso();
    const enabled = input.enabled !== false;
    const provider: OIDCProvider = {
      id: createId("oidc_provider"),
      type: input.type,
      protocol: definition.protocol,
      name: normalizeString(input.name) || definition.default_name,
      enabled,
      connection_config: normalizeProviderConnectionConfig(
        input.type,
        input.connection_config,
        { allowIncomplete: !enabled },
      ),
      created_at: now,
      updated_at: now,
      last_test_status: "idle",
    };
    await oidcRedisStore.saveProvider(provider);
    return maskProvider(provider);
  }

  async updateProvider(id: string, input: OIDCProviderUpdateInput) {
    const provider = await oidcRedisStore.getProvider(id);
    if (!provider) throw new Error(oidcT("providerNotFound"));
    const connectionPatch = input.connection_config || {};
    const nextEnabled =
      typeof input.enabled === "boolean" ? input.enabled : provider.enabled;
    const nextConnection = normalizeProviderConnectionConfig(
      provider.type,
      {
        ...provider.connection_config,
        ...connectionPatch,
      },
      { allowIncomplete: !nextEnabled },
    );
    const nextProvider: OIDCProvider = {
      ...provider,
      name:
        input.name !== undefined
          ? normalizeString(input.name) || provider.name
          : provider.name,
      enabled: nextEnabled,
      connection_config: nextConnection,
      updated_at: nowIso(),
    };
    await oidcRedisStore.saveProvider(nextProvider);
    return maskProvider(nextProvider);
  }

  async deleteProvider(id: string) {
    const provider = await oidcRedisStore.getProvider(id);
    if (!provider) throw new Error(oidcT("providerNotFound"));
    await oidcRedisStore.deleteProvider(id);
  }

  async testProvider(id: string) {
    const provider = await oidcRedisStore.getProvider(id);
    if (!provider) throw new Error(oidcT("providerNotFound"));
    let success = false;
    let message = oidcT("connectionTestSuccess");
    try {
      assertProviderReady(provider);
      if (provider.protocol === "oidc") {
        await getDiscovery(provider);
      } else {
        const cfg = provider.connection_config;
        if (!cfg.authorization_endpoint || !cfg.token_endpoint) {
          throw new Error(oidcT("oauthEndpointIncomplete"));
        }
      }
      success = true;
    } catch (error) {
      message = error instanceof Error ? error.message : oidcT("connectionTestFailed");
    }
    const updated: OIDCProvider = {
      ...provider,
      last_test_at: nowIso(),
      last_test_status: success ? "success" : "failed",
      last_error: success ? null : message,
      updated_at: nowIso(),
    };
    await oidcRedisStore.saveProvider(updated);
    return { success, message };
  }

  async createInvite(args: {
    request: Request;
    totpId: string;
    providerId: string;
    ttlSeconds?: number;
    note?: string;
  }) {
    const totp = (await configManager.getTOTPCredentials()).find(
      (item) => item.id === args.totpId,
    );
    if (!totp) throw new Error(oidcT("totpMissing"));
    const providerId = normalizeString(args.providerId);
    if (!providerId) {
      throw new Error(oidcT("selectProvider"));
    }
    const provider = await oidcRedisStore.getProvider(providerId);
    if (!provider) throw new Error(oidcT("providerNotFound"));
    if (
      !provider.enabled ||
      getMissingProviderRequiredFields(provider).length
    ) {
      throw new Error(oidcT("providerUnavailable"));
    }
    const ttlSeconds = Math.min(
      Math.max(60, Math.floor(args.ttlSeconds || DEFAULT_INVITE_TTL_SECONDS)),
      MAX_INVITE_TTL_SECONDS,
    );
    const token = createPublicToken();
    const tokenHash = hashOIDCToken(token);
    const now = Date.now();
    const invite: OIDCBindInvite = {
      token_hash: tokenHash,
      totp_id: args.totpId,
      provider_id: providerId,
      created_at: new Date(now).toISOString(),
      expires_at: new Date(now + ttlSeconds * 1000).toISOString(),
      ...(normalizeString(args.note)
        ? { note: normalizeString(args.note) }
        : {}),
    };
    await oidcRedisStore.saveInvite(invite, ttlSeconds);
    const config = await configManager.getConfig();
    return {
      invite,
      token,
      invite_url: buildInviteUrl(token, args.request, config),
    };
  }

  async inspectInvite(token: string) {
    const tokenHash = hashOIDCToken(token);
    const invite = await oidcRedisStore.getInvite(tokenHash);
    if (!invite) return null;
    if (Date.parse(invite.expires_at) <= Date.now() || invite.used_at) {
      return null;
    }
    const [totps, providers] = await Promise.all([
      configManager.getTOTPCredentials(),
      oidcRedisStore.listProviders(),
    ]);
    const totp = totps.find((item) => item.id === invite.totp_id);
    if (!totp) return null;
    const allowedProviders = providers
      .filter((provider) => provider.enabled)
      .filter(
        (provider) => getMissingProviderRequiredFields(provider).length === 0,
      )
      .filter((provider) =>
        invite.provider_id ? provider.id === invite.provider_id : true,
      )
      .map((provider) => ({
        id: provider.id,
        type: provider.type,
        name: provider.name,
        protocol: provider.protocol,
      }));
    return {
      totp: { id: totp.id, comment: totp.comment },
      provider_id: invite.provider_id,
      expires_at: invite.expires_at,
      note: invite.note,
      providers: allowedProviders,
    };
  }

  async listBindingsByTotp(totpId: string) {
    const [bindings, providers, totps] = await Promise.all([
      oidcRedisStore.listBindingsByTotp(totpId),
      oidcRedisStore.listProviders(),
      configManager.getTOTPCredentials(),
    ]);
    return bindings.map((binding) => ({
      ...binding,
      provider_name:
        providers.find((provider) => provider.id === binding.provider_id)
          ?.name || binding.provider_type,
      totp_name: totps.find((totp) => totp.id === binding.totp_id)?.comment,
    }));
  }

  async deleteBinding(id: string) {
    const deleted = await oidcRedisStore.deleteBinding(id);
    if (!deleted) throw new Error(oidcT("bindingNotFound"));
  }

  async deleteBindingsByTotp(totpId: string) {
    return oidcRedisStore.deleteBindingsByTotp(totpId);
  }

  async buildAuthorizationUrl(args: {
    request: Request;
    providerId: string;
    mode: OIDCAuthStateMode;
    redirectUri?: string;
    inviteToken?: string;
    rememberMe?: boolean;
    clientIp?: string;
  }) {
    const [provider, config] = await Promise.all([
      oidcRedisStore.getProvider(args.providerId),
      configManager.getConfig(),
    ]);
    if (!provider || !provider.enabled) {
      throw new Error(oidcT("providerUnavailable"));
    }
    assertProviderReady(provider);
    let inviteTokenHash: string | undefined;
    if (args.mode === "bind") {
      const token = normalizeString(args.inviteToken);
      if (!token) throw new Error(oidcT("inviteInvalid"));
      const invite = await this.inspectInvite(token);
      if (!invite) throw new Error(oidcT("inviteExpired"));
      if (invite.provider_id && invite.provider_id !== provider.id) {
        throw new Error(oidcT("inviteProviderNotAllowed"));
      }
      inviteTokenHash = hashOIDCToken(token);
    }
    const callbackUrl = buildCallbackUrl(provider.id, args.request, config);
    const state = createPublicToken();
    const stateHash = hashOIDCToken(state);
    const nonce =
      provider.protocol === "oidc" ? createPublicToken() : undefined;
    const codeVerifier =
      provider.protocol === "oidc" ? createPkceVerifier() : undefined;
    const safeRedirectUri = resolveSafeRedirectUri({
      config,
      request: args.request,
      redirectUri: args.redirectUri,
    });
    const authState: OIDCAuthState = {
      state_hash: stateHash,
      mode: args.mode,
      provider_id: provider.id,
      ...(safeRedirectUri ? { redirect_uri: safeRedirectUri } : {}),
      ...(inviteTokenHash ? { invite_token_hash: inviteTokenHash } : {}),
      ...(codeVerifier ? { code_verifier: codeVerifier } : {}),
      ...(nonce ? { nonce } : {}),
      remember_me: args.rememberMe === true,
      ...(args.clientIp ? { client_ip: args.clientIp } : {}),
      created_at: nowIso(),
      expires_at: new Date(Date.now() + STATE_TTL_SECONDS * 1000).toISOString(),
    };
    await oidcRedisStore.saveState(authState, STATE_TTL_SECONDS);
    const authorizationUrl =
      provider.protocol === "oidc"
        ? await this.buildStandardOidcAuthorizationUrl(
            provider,
            callbackUrl,
            state,
            nonce || "",
            codeVerifier || "",
          )
        : this.buildOauthProfileAuthorizationUrl(provider, callbackUrl, state);
    return {
      authorization_url: authorizationUrl,
      flow_token: stateHash,
      max_age: STATE_TTL_SECONDS,
    };
  }

  async consumeCallbackState(args: {
    providerId: string;
    state: string;
    flowToken?: string | null;
  }): Promise<OIDCAuthState | null> {
    if (!isOIDCFlowTokenValid(args.state, args.flowToken)) {
      return null;
    }
    const stateHash = hashOIDCToken(args.state);
    const authState = await oidcRedisStore.consumeState(stateHash);
    if (!authState || authState.provider_id !== args.providerId) {
      return null;
    }
    return authState;
  }

  async createLoginErrorNotice(message: string) {
    const token = createPublicToken();
    const tokenHash = hashOIDCToken(token);
    const createdAt = new Date();
    const expiresAt = new Date(
      createdAt.getTime() + LOGIN_ERROR_TTL_SECONDS * 1000,
    );
    await oidcRedisStore.saveLoginErrorNotice(
      {
        token_hash: tokenHash,
        message: normalizeLoginErrorMessage(message),
        created_at: createdAt.toISOString(),
        expires_at: expiresAt.toISOString(),
      },
      LOGIN_ERROR_TTL_SECONDS,
    );
    return { token, maxAge: LOGIN_ERROR_TTL_SECONDS };
  }

  async consumeLoginErrorNotice(token?: string | null) {
    const normalizedToken = normalizeString(token);
    if (!normalizedToken) return null;

    const notice = await oidcRedisStore.consumeLoginErrorNotice(
      hashOIDCToken(normalizedToken),
    );
    return notice?.message || null;
  }

  private async buildStandardOidcAuthorizationUrl(
    provider: OIDCProvider,
    callbackUrl: string,
    state: string,
    nonce: string,
    codeVerifier: string,
  ) {
    const discovery = await getDiscovery(provider);
    const cfg = provider.connection_config;
    const params = new URLSearchParams({
      client_id: cfg.client_id,
      response_type: "code",
      redirect_uri: callbackUrl,
      scope: (cfg.scopes || ["openid", "profile", "email"]).join(" "),
      state,
      nonce,
      code_challenge: createPkceChallenge(codeVerifier),
      code_challenge_method: "S256",
    });
    applyExtraAuthParams(params, cfg.extra_auth_params);
    return `${discovery.authorization_endpoint}?${params.toString()}`;
  }

  private buildOauthProfileAuthorizationUrl(
    provider: OIDCProvider,
    callbackUrl: string,
    state: string,
  ) {
    const cfg = provider.connection_config;
    if (!cfg.authorization_endpoint) {
      throw new Error(oidcT("authorizationEndpointMissing"));
    }
    const params = new URLSearchParams({
      client_id: cfg.client_id,
      response_type: "code",
      redirect_uri: callbackUrl,
      scope: (cfg.scopes || []).join(" "),
      state,
    });
    applyExtraAuthParams(params, cfg.extra_auth_params);
    return `${cfg.authorization_endpoint}?${params.toString()}`;
  }

  async resolveCallback(args: {
    request: Request;
    providerId: string;
    code: string;
    state: string;
    flowToken?: string | null;
  }) {
    assertOIDCFlowTokenValid(args.state, args.flowToken);
    const stateHash = hashOIDCToken(args.state);
    const authState = await oidcRedisStore.consumeState(stateHash);
    if (!authState || authState.provider_id !== args.providerId) {
      throw new Error(OIDC_CALLBACK_STATE_EXPIRED_MESSAGE);
    }
    const provider = await oidcRedisStore.getProvider(args.providerId);
    if (!provider || !provider.enabled) {
      throw new Error(oidcT("providerUnavailable"));
    }
    const config = await configManager.getConfig();
    const callbackUrl = buildCallbackUrl(provider.id, args.request, config);
    const profile =
      provider.protocol === "oidc"
        ? await this.resolveStandardOidcCallback(
            provider,
            args.code,
            callbackUrl,
            authState,
          )
        : await this.resolveOauthProfileCallback(
            provider,
            args.code,
            callbackUrl,
          );
    const subjectKey = buildSubjectKey(
      provider.id,
      profile.issuer,
      profile.subject,
    );
    if (authState.mode === "bind") {
      if (!authState.invite_token_hash) {
        throw new Error(oidcT("bindStateInvalid"));
      }
      return this.bindProfileAndResolveLogin({
        provider,
        profile,
        subjectKey,
        state: authState,
      });
    }
    const binding = await oidcRedisStore.getBindingBySubject(subjectKey);
    if (!binding) {
      throw new Error(oidcT("accountNotBoundCannotLogin"));
    }
    await oidcRedisStore.saveBinding({
      ...binding,
      display_name: profile.display_name || binding.display_name,
      email: profile.email || binding.email,
      email_verified: profile.email_verified ?? binding.email_verified,
      avatar_url: profile.avatar_url || binding.avatar_url,
      last_used_at: nowIso(),
      updated_at: nowIso(),
    });
    return {
      state: authState,
      provider,
      binding,
      profile,
    };
  }

  private async resolveStandardOidcCallback(
    provider: OIDCProvider,
    code: string,
    callbackUrl: string,
    authState: OIDCAuthState,
  ) {
    const discovery = await getDiscovery(provider);
    const cfg = provider.connection_config;
    const body = new URLSearchParams({
      grant_type: "authorization_code",
      client_id: cfg.client_id,
      client_secret: cfg.client_secret,
      code,
      redirect_uri: callbackUrl,
    });
    if (authState.code_verifier) {
      body.set("code_verifier", authState.code_verifier);
    }
    const tokenPayload = await exchangeFormToken(
      discovery.token_endpoint,
      body,
    );
    return verifyStandardOidcProfile(
      provider,
      tokenPayload,
      discovery,
      authState.nonce,
    );
  }

  private async resolveOauthProfileCallback(
    provider: OIDCProvider,
    code: string,
    callbackUrl: string,
  ) {
    const cfg = provider.connection_config;
    if (!cfg.token_endpoint) throw new Error(oidcT("tokenEndpointMissing"));
    const body = new URLSearchParams({
      grant_type: "authorization_code",
      client_id: cfg.client_id,
      client_secret: cfg.client_secret,
      code,
      redirect_uri: callbackUrl,
    });
    const tokenPayload = await exchangeFormToken(
      cfg.token_endpoint,
      body,
      provider.type === "github" ? { Accept: "application/json" } : undefined,
    );
    const accessToken = parseAccessToken(tokenPayload);
    if (provider.type === "github") {
      return fetchGithubProfile(provider, accessToken);
    }
    throw new Error(oidcT("providerUnsupported"));
  }

  private async bindProfileAndResolveLogin(args: {
    provider: OIDCProvider;
    profile: ExternalAuthProfile;
    subjectKey: string;
    state: OIDCAuthState;
  }) {
    const invite = await oidcRedisStore.getInvite(
      args.state.invite_token_hash!,
    );
    if (!invite) throw new Error(oidcT("inviteExpired"));
    if (invite.provider_id && invite.provider_id !== args.provider.id) {
      throw new Error(oidcT("bindProviderMismatch"));
    }
    const totp = (await configManager.getTOTPCredentials()).find(
      (item) => item.id === invite.totp_id,
    );
    if (!totp) throw new Error(oidcT("inviteTotpMissing"));
    const existing = await oidcRedisStore.getBindingBySubject(args.subjectKey);
    if (existing && existing.totp_id !== invite.totp_id) {
      throw new Error(oidcT("accountAlreadyBoundOtherTotp"));
    }
    const consumed = await oidcRedisStore.consumeInvite(
      args.state.invite_token_hash!,
    );
    if (!consumed) throw new Error(oidcT("inviteUsed"));
    if (existing) {
      const updated: OIDCBinding = {
        ...existing,
        display_name: args.profile.display_name || existing.display_name,
        email: args.profile.email || existing.email,
        email_verified: args.profile.email_verified ?? existing.email_verified,
        avatar_url: args.profile.avatar_url || existing.avatar_url,
        last_used_at: nowIso(),
        updated_at: nowIso(),
      };
      await oidcRedisStore.saveBinding(updated);
      return {
        state: args.state,
        provider: args.provider,
        binding: updated,
        profile: args.profile,
      };
    }
    const now = nowIso();
    const binding: OIDCBinding = {
      id: createId("oidc_binding"),
      provider_id: args.provider.id,
      provider_type: args.provider.type,
      totp_id: invite.totp_id,
      issuer: args.profile.issuer,
      subject: args.profile.subject,
      subject_key: args.subjectKey,
      ...(args.profile.display_name
        ? { display_name: args.profile.display_name }
        : {}),
      ...(args.profile.email ? { email: args.profile.email } : {}),
      ...(typeof args.profile.email_verified === "boolean"
        ? { email_verified: args.profile.email_verified }
        : {}),
      ...(args.profile.avatar_url
        ? { avatar_url: args.profile.avatar_url }
        : {}),
      created_at: now,
      updated_at: now,
      last_used_at: now,
    };
    const saved = await oidcRedisStore.saveBindingIfSubjectAvailable(binding);
    if (!saved) {
      const raced = await oidcRedisStore.getBindingBySubject(args.subjectKey);
      if (raced && raced.totp_id === invite.totp_id) {
        const updated: OIDCBinding = {
          ...raced,
          display_name: args.profile.display_name || raced.display_name,
          email: args.profile.email || raced.email,
          email_verified: args.profile.email_verified ?? raced.email_verified,
          avatar_url: args.profile.avatar_url || raced.avatar_url,
          last_used_at: now,
          updated_at: now,
        };
        await oidcRedisStore.saveBinding(updated);
        return {
          state: args.state,
          provider: args.provider,
          binding: updated,
          profile: args.profile,
        };
      }
      throw new Error(oidcT("accountAlreadyBoundOtherTotp"));
    }
    return {
      state: args.state,
      provider: args.provider,
      binding,
      profile: args.profile,
    };
  }
}

export const oidcAuthService = new OIDCAuthService();
