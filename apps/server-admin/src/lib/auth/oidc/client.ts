import { createRemoteJWKSet, jwtVerify, type JWTPayload } from "jose";
import { oidcT } from "./messages";
import { normalizeOptionalString, normalizeString } from "./strings";
import { buildOidcDiscoveryUrl } from "./urls";
import type {
  ExternalAuthProfile,
  OIDCDiscoveryDocument,
  OIDCProvider,
} from "./types";

const REQUEST_TIMEOUT_MS = 7000;
const remoteJwksCache = new Map<
  string,
  ReturnType<typeof createRemoteJWKSet>
>();

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

export const parseAccessToken = (payload: Record<string, unknown>) => {
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

export const getDiscovery = async (
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

export const exchangeFormToken = async (
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

export const verifyStandardOidcProfile = async (
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

export const fetchGithubProfile = async (
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
