import {
  getDefaultConnectionConfig,
  getOIDCProviderDefinition,
} from "./catalog";
import { oidcT } from "./messages";
import {
  normalizeOptionalString,
  normalizeString,
  normalizeStringRecord,
} from "./strings";
import type {
  ExternalAuthProviderType,
  OIDCProvider,
  OIDCProviderConnectionConfig,
  OIDCProviderView,
} from "./types";

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

const assertExtraAuthParamKeyAllowed = (key: string) => {
  const normalizedKey = key.trim().toLowerCase();
  if (RESERVED_EXTRA_AUTH_PARAM_KEYS.has(normalizedKey)) {
    throw new Error(oidcT("reservedExtraAuthParam", { key }));
  }
};

export const assertExtraAuthParamsAllowed = (
  extraParams?: Record<string, string>,
) => {
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

export const applyExtraAuthParams = (
  params: URLSearchParams,
  extraParams?: Record<string, string>,
) => {
  assertExtraAuthParamsAllowed(extraParams);
  for (const [key, value] of Object.entries(extraParams || {})) {
    params.set(key, value);
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

export const normalizeProviderConnectionConfig = (
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

export const getMissingProviderRequiredFields = (provider: OIDCProvider) => {
  const definition = getOIDCProviderDefinition(provider.type);
  if (!definition) return ["type"];
  return definition.required_fields.filter(
    (field) => !isConnectionValuePresent(provider.connection_config[field]),
  );
};

export const assertProviderReady = (provider: OIDCProvider) => {
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

export const maskProvider = (
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
