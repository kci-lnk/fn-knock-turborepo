import Redis from "ioredis";
import {
  X509Certificate,
  createHash,
  createPrivateKey,
  randomBytes,
} from "node:crypto";
import { homedir } from "node:os";
import { spawn } from "node:child_process";
import { dataPath } from "./AppDirManager";
import { ACME_EXECUTABLE_PATH, ACME_HOME_DIR } from "./acme-paths";
import {
  DEFAULT_ACME_CERTIFICATE_AUTHORITY,
  normalizeAcmeCertificateAuthority,
  type AcmeCertificateAuthority,
} from "./acme-certificate-authority";
import {
  DEFAULT_REDIS_LOG_BUFFER_MAX_LEN,
  RedisLogBuffer,
} from "./redis-log-buffer";
import { collectStreamOutput, fileExists, waitForProcessExit } from "./runtime";
import { isAuthServiceTarget } from "./auth-service";
import {
  DEFAULT_TERMINAL_FEATURE_CONFIG,
  type TerminalFeatureConfig,
  normalizeTerminalFeatureConfig,
} from "./terminal-shared";
import {
  DEFAULT_REVERSE_PROXY_SUBMODE,
  normalizeReverseProxySubmode,
  type ReverseProxySubmode,
} from "./reverse-proxy-submode";

const REDIS_CONFIG = {
  host: process.env.REDIS_HOST || "127.0.0.1",
  port: parseInt(process.env.REDIS_PORT || "6379"),
  password: process.env.REDIS_PASSWORD,
};

const parseEnvInt = (value: string | undefined, fallback: number): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  return Number.isFinite(parsed) ? parsed : fallback;
};

const ACME_RUNTIME_LOCK_TTL_SECONDS = Math.max(
  300,
  Math.min(6 * 60 * 60, parseEnvInt(process.env.ACME_RUNTIME_LOCK_TTL, 900)),
);

export const redis = new Redis(REDIS_CONFIG);
redis.on("error", (err) => {
  console.error("Redis connection error:", err);
});

export interface ProxyMapping {
  path: string;
  target: string;
  rewrite_html: boolean;
  use_auth: boolean;
  use_root_mode: boolean;
  strip_path: boolean;
}

export type RunType = 0 | 1 | 3;

export type HostAccessMode = "login_first" | "strict_whitelist";
export type HostServiceRole = "app" | "auth";
export type StreamMappingProtocol = "tcp" | "udp";

export interface HostMapping {
  host: string;
  target: string;
  use_auth: boolean;
  access_mode: HostAccessMode;
  suppress_toolbar: boolean;
  preserve_host: boolean;
  service_role: HostServiceRole;
  title: string;
  title_override: string;
  favicon: string;
}

export interface StreamMapping {
  protocol: StreamMappingProtocol;
  listen_port: number;
  target: string;
  use_auth: boolean;
}

export type PasskeyRpMode = "auth_host" | "parent_domain";

export interface SubdomainModeConfig {
  root_domain: string;
  auth_host: string;
  auth_target: string;
  cookie_domain: string;
  public_auth_base_url: string;
  public_http_port?: number;
  public_https_port?: number;
  auth_cache_ttl_seconds: number;
  auth_cache_unauthorized_ttl_seconds: number;
  default_access_mode: HostAccessMode;
  auto_add_whitelist_on_login: boolean;
  passkey_rp_mode: PasskeyRpMode;
  passkey_rp_id?: string;
}

export interface SSLConfig {
  cert: string;
  key: string;
  active_cert_id?: string;
  deployment_mode?: SSLDeploymentMode;
  certificates?: SSLManagedCertificate[];
}

export interface SSLCertInfo {
  issuer: string;
  subject: string;
  validFrom: string;
  validTo: string;
  dnsNames: string[];
  serialNumber: string;
}

export type SSLDeploymentMode = "single_active" | "multi_sni";
export type SSLCertificateSource = "manual" | "acme" | "ca";

export interface SSLManagedCertificate {
  id: string;
  label: string;
  source: SSLCertificateSource;
  primary_domain?: string;
  source_ref_id?: string;
  cert: string;
  key: string;
  created_at: string;
  updated_at: string;
}

export interface SSLCertificateSummary {
  id: string;
  label: string;
  source: SSLCertificateSource;
  primary_domain?: string;
  source_ref_id?: string;
  created_at: string;
  updated_at: string;
  certInfo?: SSLCertInfo;
  is_active: boolean;
}

export interface SSLStatus {
  enabled: boolean;
  activeCertId?: string;
  deploymentMode: SSLDeploymentMode;
  certInfo?: SSLCertInfo;
  certificates: SSLCertificateSummary[];
}

export interface FnosShareBypassConfig {
  enabled: boolean;
  upstream_timeout_ms: number;
  validation_cache_ttl_seconds: number;
  validation_lock_ttl_seconds: number;
  session_ttl_seconds: number;
}

export interface GatewayLoggingSettings {
  enabled: boolean;
  max_days: number;
}

export interface ReverseProxyThrottleConfig {
  enabled: boolean;
  requests_per_second: number;
  burst: number;
  block_seconds: number;
}

export interface ProtocolMappingFeatureConfig {
  enabled: boolean;
}

export type CaptchaProvider = "pow" | "turnstile";

export type CaptchaWidgetMode = "normal";

export type TurnstileCaptchaConfig = {
  site_key: string;
  secret_key: string;
};

export type CaptchaSettings = {
  provider: CaptchaProvider;
  widget_mode: CaptchaWidgetMode;
  pow: Record<string, never>;
  turnstile: TurnstileCaptchaConfig;
};

export type AcmeJobStatus = "queued" | "running" | "succeeded" | "failed";
export type AcmeJobMethod = "dns" | "http" | "https";
export type AcmeJobTrigger = "manual_request" | "auto_renew";
export type AcmeApplicationLatestJobStatus = AcmeJobStatus | "idle";
export type AcmeJob = {
  id: string;
  applicationId?: string;
  domains: string[];
  method: AcmeJobMethod;
  provider: string | null;
  trigger?: AcmeJobTrigger;
  createdAt: string;
  startedAt?: string;
  finishedAt?: string;
  status: AcmeJobStatus;
  progress: number;
  message?: string;
};

export type AcmeApplication = {
  id: string;
  name?: string;
  domains: string[];
  primaryDomain: string;
  dnsType: string;
  credentials: Record<string, string>;
  renewEnabled: boolean;
  createdAt: string;
  updatedAt: string;
  latestJobId?: string;
  latestJobStatus?: AcmeApplicationLatestJobStatus;
  latestJobTrigger?: AcmeJobTrigger;
  latestJobAt?: string;
  lastError?: string;
};

export type AcmeIssuedCertificate = {
  applicationId: string;
  primaryDomain: string;
  cert: string;
  key: string;
  certInfo: SSLCertInfo;
  createdAt: string;
  updatedAt: string;
  libraryCertificateId?: string;
  libraryLinkedAt?: string;
};

export type AcmeRuntimeLock = {
  locked: boolean;
  lockId?: string;
  jobId?: string;
  applicationId?: string;
  reason?: AcmeJobTrigger;
  startedAt?: string;
  heartbeatAt?: string;
  expiresAt?: string;
};

export type AcmeApplicationSaveResult = {
  application: AcmeApplication;
  certificateInvalidated: boolean;
  deletedIssuedCertificate: AcmeIssuedCertificate | null;
  removedLibraryCertificates: SSLManagedCertificate[];
  removedActiveLibraryCertificate: boolean;
  removedDomains: string[];
};

export type AcmeSettings = {
  domains: string[];
  dnsType: string;
  credentials: Record<string, string>;
  updatedAt: string;
};

export type AcmeClientSettings = {
  certificateAuthority: AcmeCertificateAuthority;
  updatedAt: string;
};

export type LoginSession = {
  totpId: string;
  method: "TOTP" | "PASSKEY";
  credentialId: string;
  credentialName: string;
  grantType?: "browser_session" | "login_ip_grant";
  postLoginIpGrantMode?: PostLoginIpGrantMode | null;
  postLoginIpGrantRecordId?: string | null;
  comment?: string;
  ip: string;
  userAgent: string;
  loginTime: string;
  expiresAt?: string;
  ipLocation?: string;
};

export interface AppConfig {
  run_type: RunType;
  reverse_proxy_submode: ReverseProxySubmode;
  whitelist_ips: string[];
  proxy_mappings: ProxyMapping[];
  host_mappings: HostMapping[];
  stream_mappings: StreamMapping[];
  subdomain_mode: SubdomainModeConfig;
  ssl: SSLConfig;
  default_route: string;
  default_tunnel?: "frp" | "cloudflared";
  fnos_share_bypass?: FnosShareBypassConfig;
  gateway_logging?: GatewayLoggingSettings;
  reverse_proxy_throttle?: ReverseProxyThrottleConfig;
  auth_credential_settings?: AuthCredentialSettings;
  terminal_feature?: TerminalFeatureConfig;
}

export interface RunModePromptPreferences {
  directToReverseProxy: boolean;
  reverseProxyToDirect: boolean;
  switchToSubdomain: boolean;
  subdomainToReverseProxy: boolean;
}

export type PostLoginIpGrantMode = "follow_session" | "disabled" | "custom";

export interface AuthCredentialSettings {
  session_ttl_seconds: number;
  remember_me_ttl_seconds: number;
  post_login_ip_grant_mode: PostLoginIpGrantMode;
  post_login_ip_grant_ttl_seconds: number | null;
}

export const DEFAULT_AUTH_CREDENTIAL_SETTINGS: AuthCredentialSettings = {
  session_ttl_seconds: 24 * 3600,
  remember_me_ttl_seconds: 365 * 24 * 3600,
  post_login_ip_grant_mode: "follow_session",
  post_login_ip_grant_ttl_seconds: 3600,
};

const DEFAULT_GATEWAY_LOGGING_SETTINGS: GatewayLoggingSettings = {
  enabled: false,
  max_days: 7,
};

export const DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG: ReverseProxyThrottleConfig =
  {
    enabled: true,
    requests_per_second: 100,
    burst: 200,
    block_seconds: 30,
  };

const LEGACY_REVERSE_PROXY_THROTTLE_PATCH_FLAG_KEY =
  "fn_knock:patch:reverse-proxy-throttle:v1";

const LEGACY_REVERSE_PROXY_THROTTLE_CONFIG: Pick<
  ReverseProxyThrottleConfig,
  "requests_per_second" | "burst" | "block_seconds"
> = {
  requests_per_second: 20,
  burst: 50,
  block_seconds: 30,
};

const DEFAULT_PROTOCOL_MAPPING_FEATURE_CONFIG: ProtocolMappingFeatureConfig = {
  enabled: false,
};

export type TOTPCredential = {
  id: string;
  secret: string;
  comment: string;
  createdAt: string;
};

export type PasskeyCredential = {
  id: string;
  totpId: string;
  publicKey: string;
  counter: number;
  transports?: string[];
  deviceName: string;
  createdAt: string;
  lastUsedAt?: string;
};

const DEFAULT_ROUTE_PLACEHOLDER = "/__select__";

const DEFAULT_CONFIG: AppConfig = {
  run_type: 1,
  reverse_proxy_submode: DEFAULT_REVERSE_PROXY_SUBMODE,
  whitelist_ips: [],
  proxy_mappings: [],
  host_mappings: [],
  stream_mappings: [],
  subdomain_mode: {
    root_domain: "",
    auth_host: "",
    auth_target: `http://localhost:${process.env.AUTH_PORT || "7997"}`,
    cookie_domain: "",
    public_auth_base_url: "",
    public_http_port: 0,
    public_https_port: 0,
    auth_cache_ttl_seconds: 1,
    auth_cache_unauthorized_ttl_seconds: 1,
    default_access_mode: "login_first",
    auto_add_whitelist_on_login: true,
    passkey_rp_mode: "auth_host",
    passkey_rp_id: "",
  },
  ssl: {
    cert: "",
    key: "",
    active_cert_id: "",
    deployment_mode: "single_active",
    certificates: [],
  },
  default_route: DEFAULT_ROUTE_PLACEHOLDER,
  default_tunnel: "frp",
  fnos_share_bypass: {
    enabled: false,
    upstream_timeout_ms: 2500,
    validation_cache_ttl_seconds: 30,
    validation_lock_ttl_seconds: 5,
    session_ttl_seconds: 300,
  },
  gateway_logging: {
    ...DEFAULT_GATEWAY_LOGGING_SETTINGS,
  },
  reverse_proxy_throttle: {
    ...DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG,
  },
  auth_credential_settings: {
    ...DEFAULT_AUTH_CREDENTIAL_SETTINGS,
  },
  terminal_feature: {
    ...DEFAULT_TERMINAL_FEATURE_CONFIG,
  },
};

const DEFAULT_RUN_MODE_PROMPT_PREFERENCES: RunModePromptPreferences = {
  directToReverseProxy: false,
  reverseProxyToDirect: false,
  switchToSubdomain: false,
  subdomainToReverseProxy: false,
};

const DEFAULT_FNOS_SHARE_BYPASS_CONFIG: FnosShareBypassConfig = {
  enabled: false,
  upstream_timeout_ms: 2500,
  validation_cache_ttl_seconds: 30,
  validation_lock_ttl_seconds: 5,
  session_ttl_seconds: 300,
};

const normalizeGatewayLoggingSettings = (
  value?: Partial<GatewayLoggingSettings> | null,
): GatewayLoggingSettings => {
  const raw = value ?? {};

  return {
    enabled: raw.enabled === true,
    max_days: normalizePositiveInt(
      raw.max_days,
      DEFAULT_GATEWAY_LOGGING_SETTINGS.max_days,
    ),
  };
};

const normalizeReverseProxyThrottleConfig = (
  value?: Partial<ReverseProxyThrottleConfig> | null,
): ReverseProxyThrottleConfig => {
  const raw = value ?? {};

  return {
    enabled:
      typeof raw.enabled === "boolean"
        ? raw.enabled
        : DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG.enabled,
    requests_per_second: normalizePositiveInt(
      raw.requests_per_second,
      DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG.requests_per_second,
    ),
    burst: normalizePositiveInt(
      raw.burst,
      DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG.burst,
    ),
    block_seconds: normalizePositiveInt(
      raw.block_seconds,
      DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG.block_seconds,
    ),
  };
};

const normalizeProtocolMappingFeatureConfig = (
  value?: Partial<ProtocolMappingFeatureConfig> | null,
): ProtocolMappingFeatureConfig => {
  const raw = value ?? {};

  return {
    enabled: raw.enabled === true,
  };
};

const DEFAULT_CAPTCHA_SETTINGS: CaptchaSettings = {
  provider: "pow",
  widget_mode: "normal",
  pow: {},
  turnstile: {
    site_key: "",
    secret_key: "",
  },
};

const normalizePositiveInt = (
  value: unknown,
  fallback: number,
  {
    min = 1,
    max = Number.MAX_SAFE_INTEGER,
  }: { min?: number; max?: number } = {},
): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(max, Math.max(min, parsed));
};

const normalizeFnosShareBypassConfig = (
  value?: Partial<FnosShareBypassConfig> | null,
): FnosShareBypassConfig => {
  const raw = value ?? {};

  return {
    enabled: raw.enabled === true,
    upstream_timeout_ms: normalizePositiveInt(
      raw.upstream_timeout_ms,
      DEFAULT_FNOS_SHARE_BYPASS_CONFIG.upstream_timeout_ms,
      { min: 500, max: 15000 },
    ),
    validation_cache_ttl_seconds: normalizePositiveInt(
      raw.validation_cache_ttl_seconds,
      DEFAULT_FNOS_SHARE_BYPASS_CONFIG.validation_cache_ttl_seconds,
      { min: 5, max: 300 },
    ),
    validation_lock_ttl_seconds: normalizePositiveInt(
      raw.validation_lock_ttl_seconds,
      DEFAULT_FNOS_SHARE_BYPASS_CONFIG.validation_lock_ttl_seconds,
      { min: 1, max: 30 },
    ),
    session_ttl_seconds: normalizePositiveInt(
      raw.session_ttl_seconds,
      DEFAULT_FNOS_SHARE_BYPASS_CONFIG.session_ttl_seconds,
      { min: 30, max: 3600 },
    ),
  };
};

const normalizePostLoginIpGrantMode = (
  value: unknown,
  legacyAutoAddWhitelistOnLogin?: boolean | null,
): PostLoginIpGrantMode => {
  if (
    value === "follow_session" ||
    value === "disabled" ||
    value === "custom"
  ) {
    return value;
  }

  if (legacyAutoAddWhitelistOnLogin === false) {
    return "disabled";
  }

  return DEFAULT_AUTH_CREDENTIAL_SETTINGS.post_login_ip_grant_mode;
};

const normalizeAuthCredentialSettings = (
  value?: Partial<AuthCredentialSettings> | null,
  options?: {
    legacyAutoAddWhitelistOnLogin?: boolean | null;
  },
): AuthCredentialSettings => {
  const raw = value ?? {};
  const sessionTtlSeconds = normalizePositiveInt(
    raw.session_ttl_seconds,
    DEFAULT_AUTH_CREDENTIAL_SETTINGS.session_ttl_seconds,
    { min: 60, max: 5 * 365 * 24 * 3600 },
  );
  const rememberMeTtlSeconds = normalizePositiveInt(
    raw.remember_me_ttl_seconds,
    DEFAULT_AUTH_CREDENTIAL_SETTINGS.remember_me_ttl_seconds,
    { min: sessionTtlSeconds, max: 5 * 365 * 24 * 3600 },
  );
  const postLoginIpGrantMode = normalizePostLoginIpGrantMode(
    raw.post_login_ip_grant_mode,
    options?.legacyAutoAddWhitelistOnLogin,
  );
  const postLoginIpGrantTtlSeconds = normalizePositiveInt(
    raw.post_login_ip_grant_ttl_seconds,
    DEFAULT_AUTH_CREDENTIAL_SETTINGS.post_login_ip_grant_ttl_seconds ?? 3600,
    { min: 60, max: 5 * 365 * 24 * 3600 },
  );

  return {
    session_ttl_seconds: sessionTtlSeconds,
    remember_me_ttl_seconds: rememberMeTtlSeconds,
    post_login_ip_grant_mode: postLoginIpGrantMode,
    post_login_ip_grant_ttl_seconds:
      postLoginIpGrantMode === "custom" ? postLoginIpGrantTtlSeconds : null,
  };
};

const normalizeHost = (value: unknown): string => {
  if (typeof value !== "string") return "";
  return value
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/\/.*$/, "");
};

const normalizeTimestamp = (value: unknown): string => {
  if (typeof value !== "string") return "";
  const trimmed = value.trim();
  if (!trimmed) return "";
  const parsed = Date.parse(trimmed);
  return Number.isFinite(parsed) ? new Date(parsed).toISOString() : "";
};

const normalizeStringRecord = (value: unknown): Record<string, string> => {
  if (!value || typeof value !== "object") return {};
  const next: Record<string, string> = {};
  for (const [key, rawValue] of Object.entries(
    value as Record<string, unknown>,
  )) {
    const normalizedKey = String(key ?? "").trim();
    const normalizedValue = String(rawValue ?? "").trim();
    if (!normalizedKey || !normalizedValue) continue;
    next[normalizedKey] = normalizedValue;
  }
  return next;
};

const normalizeDomainList = (value: unknown): string[] => {
  if (!Array.isArray(value)) return [];
  const domains: string[] = [];
  const seen = new Set<string>();
  for (const raw of value) {
    const domain = String(raw ?? "")
      .trim()
      .toLowerCase();
    if (!domain || seen.has(domain)) continue;
    seen.add(domain);
    domains.push(domain);
  }
  return domains;
};

const buildNormalizedDomainSignature = (domains: string[]): string =>
  [...normalizeDomainList(domains)]
    .sort((a, b) => a.localeCompare(b))
    .join("\n");

const hasSameNormalizedDomainSet = (left: string[], right: string[]): boolean =>
  buildNormalizedDomainSignature(left) ===
  buildNormalizedDomainSignature(right);

const normalizeOptionalString = (value: unknown): string | undefined => {
  if (typeof value !== "string") return undefined;
  const normalized = value.trim();
  return normalized || undefined;
};

const normalizeSSLCertInfoValue = (value: unknown): SSLCertInfo | null => {
  if (!value || typeof value !== "object") return null;
  const raw = value as Partial<SSLCertInfo>;
  const issuer = typeof raw.issuer === "string" ? raw.issuer.trim() : "";
  const subject = typeof raw.subject === "string" ? raw.subject.trim() : "";
  const validFrom =
    typeof raw.validFrom === "string" ? raw.validFrom.trim() : "";
  const validTo = typeof raw.validTo === "string" ? raw.validTo.trim() : "";
  const serialNumber =
    typeof raw.serialNumber === "string" ? raw.serialNumber.trim() : "";
  const dnsNames = normalizeDomainList(raw.dnsNames);

  if (!issuer || !subject || !validFrom || !validTo || !serialNumber) {
    return null;
  }

  return {
    issuer,
    subject,
    validFrom,
    validTo,
    dnsNames,
    serialNumber,
  };
};

const normalizeSSLCertificateSource = (
  value: unknown,
): SSLCertificateSource => {
  if (value === "acme") return "acme";
  if (value === "ca") return "ca";
  return "manual";
};

const normalizeSSLDeploymentMode = (value: unknown): SSLDeploymentMode =>
  value === "multi_sni" ? "multi_sni" : "single_active";

const buildSSLCertificateId = (cert: string, key: string): string =>
  `ssl_${createHash("sha256")
    .update(cert)
    .update("\n")
    .update(key)
    .digest("hex")
    .slice(0, 16)}`;

const normalizeCertificateLabel = ({
  value,
  primaryDomain,
  source,
}: {
  value: unknown;
  primaryDomain?: string;
  source: SSLCertificateSource;
}): string => {
  if (typeof value === "string" && value.trim()) return value.trim();
  if (primaryDomain) return primaryDomain;
  if (source === "acme") return "ACME 证书";
  if (source === "ca") return "自签发证书";
  return "手动上传证书";
};

const normalizeManagedSSLCertificate = (
  value?: Partial<SSLManagedCertificate> | null,
): SSLManagedCertificate | null => {
  const raw = value ?? {};
  const cert = typeof raw.cert === "string" ? raw.cert.trim() : "";
  const key = typeof raw.key === "string" ? raw.key.trim() : "";
  if (!cert || !key) return null;

  const source = normalizeSSLCertificateSource(raw.source);
  const primaryDomain =
    typeof raw.primary_domain === "string"
      ? raw.primary_domain.trim().toLowerCase()
      : "";
  const createdAt =
    normalizeTimestamp(raw.created_at) || "1970-01-01T00:00:00.000Z";
  const updatedAt = normalizeTimestamp(raw.updated_at) || createdAt;

  return {
    id:
      typeof raw.id === "string" && raw.id.trim()
        ? raw.id.trim()
        : buildSSLCertificateId(cert, key),
    label: normalizeCertificateLabel({
      value: raw.label,
      primaryDomain: primaryDomain || undefined,
      source,
    }),
    source,
    primary_domain: primaryDomain || undefined,
    source_ref_id: normalizeOptionalString(raw.source_ref_id),
    cert,
    key,
    created_at: createdAt,
    updated_at: updatedAt,
  };
};

const normalizeAcmeJobTrigger = (
  value: unknown,
): AcmeJobTrigger | undefined => {
  if (value === "manual_request") return "manual_request";
  if (value === "auto_renew") return "auto_renew";
  return undefined;
};

const normalizeAcmeApplicationLatestJobStatus = (
  value: unknown,
): AcmeApplicationLatestJobStatus | undefined => {
  if (value === "idle") return "idle";
  if (value === "queued") return "queued";
  if (value === "running") return "running";
  if (value === "succeeded") return "succeeded";
  if (value === "failed") return "failed";
  return undefined;
};

const normalizeAcmeJob = (value: unknown): AcmeJob | null => {
  if (!value || typeof value !== "object") return null;
  const raw = value as Partial<AcmeJob>;
  const id = typeof raw.id === "string" ? raw.id.trim() : "";
  const domains = normalizeDomainList(raw.domains);
  const createdAt = normalizeTimestamp(raw.createdAt);
  const status =
    raw.status === "queued" ||
    raw.status === "running" ||
    raw.status === "succeeded" ||
    raw.status === "failed"
      ? raw.status
      : undefined;
  if (!id || !domains.length || !createdAt || !status) return null;

  return {
    id,
    applicationId: normalizeOptionalString(raw.applicationId),
    domains,
    method:
      raw.method === "http" || raw.method === "https" ? raw.method : "dns",
    provider:
      typeof raw.provider === "string" && raw.provider.trim()
        ? raw.provider.trim()
        : null,
    trigger: normalizeAcmeJobTrigger(raw.trigger),
    createdAt,
    startedAt: normalizeOptionalString(raw.startedAt),
    finishedAt: normalizeOptionalString(raw.finishedAt),
    status,
    progress:
      typeof raw.progress === "number" && Number.isFinite(raw.progress)
        ? Math.max(0, Math.min(100, Math.round(raw.progress)))
        : 0,
    message: normalizeOptionalString(raw.message),
  };
};

const normalizeAcmeApplication = (value: unknown): AcmeApplication | null => {
  if (!value || typeof value !== "object") return null;
  const raw = value as Partial<AcmeApplication>;
  const id = typeof raw.id === "string" ? raw.id.trim() : "";
  const domains = normalizeDomainList(raw.domains);
  const primaryDomain =
    typeof raw.primaryDomain === "string"
      ? raw.primaryDomain.trim().toLowerCase()
      : domains[0] || "";
  const dnsType = typeof raw.dnsType === "string" ? raw.dnsType.trim() : "";
  const createdAt = normalizeTimestamp(raw.createdAt);
  const updatedAt = normalizeTimestamp(raw.updatedAt) || createdAt;

  if (!id || !domains.length || !primaryDomain || !dnsType || !createdAt) {
    return null;
  }

  return {
    id,
    name: normalizeOptionalString(raw.name),
    domains,
    primaryDomain,
    dnsType,
    credentials: normalizeStringRecord(raw.credentials),
    renewEnabled: raw.renewEnabled !== false,
    createdAt,
    updatedAt,
    latestJobId: normalizeOptionalString(raw.latestJobId),
    latestJobStatus: normalizeAcmeApplicationLatestJobStatus(
      raw.latestJobStatus,
    ),
    latestJobTrigger: normalizeAcmeJobTrigger(raw.latestJobTrigger),
    latestJobAt: normalizeOptionalString(raw.latestJobAt),
    lastError: normalizeOptionalString(raw.lastError),
  };
};

const normalizeAcmeIssuedCertificate = (
  value: unknown,
): AcmeIssuedCertificate | null => {
  if (!value || typeof value !== "object") return null;
  const raw = value as Partial<AcmeIssuedCertificate>;
  const applicationId =
    typeof raw.applicationId === "string" ? raw.applicationId.trim() : "";
  const primaryDomain =
    typeof raw.primaryDomain === "string"
      ? raw.primaryDomain.trim().toLowerCase()
      : "";
  const cert = typeof raw.cert === "string" ? raw.cert.trim() : "";
  const key = typeof raw.key === "string" ? raw.key.trim() : "";
  const createdAt = normalizeTimestamp(raw.createdAt);
  const updatedAt = normalizeTimestamp(raw.updatedAt) || createdAt;
  const certInfo = normalizeSSLCertInfoValue(raw.certInfo);

  if (
    !applicationId ||
    !primaryDomain ||
    !cert ||
    !key ||
    !createdAt ||
    !certInfo
  ) {
    return null;
  }

  return {
    applicationId,
    primaryDomain,
    cert,
    key,
    certInfo,
    createdAt,
    updatedAt,
    libraryCertificateId: normalizeOptionalString(raw.libraryCertificateId),
    libraryLinkedAt: normalizeOptionalString(raw.libraryLinkedAt),
  };
};

const normalizeAcmeRuntimeLock = (value: unknown): AcmeRuntimeLock => {
  if (!value || typeof value !== "object") return { locked: false };
  const raw = value as Partial<AcmeRuntimeLock>;
  if (raw.locked !== true) return { locked: false };
  return {
    locked: true,
    lockId: normalizeOptionalString(raw.lockId),
    jobId: normalizeOptionalString(raw.jobId),
    applicationId: normalizeOptionalString(raw.applicationId),
    reason: normalizeAcmeJobTrigger(raw.reason),
    startedAt: normalizeOptionalString(raw.startedAt),
    heartbeatAt: normalizeOptionalString(raw.heartbeatAt),
    expiresAt: normalizeOptionalString(raw.expiresAt),
  };
};

const findMatchingSSLCertificate = (
  certificates: SSLManagedCertificate[],
  cert: string,
  key: string,
): SSLManagedCertificate | null =>
  certificates.find((item) => item.cert === cert && item.key === key) || null;

const normalizeSSLConfig = (value?: Partial<SSLConfig> | null): SSLConfig => {
  const raw = value ?? {};
  const certificates = Array.isArray(raw.certificates)
    ? raw.certificates
        .map((item) => normalizeManagedSSLCertificate(item))
        .filter((item): item is SSLManagedCertificate => item !== null)
    : [];

  const normalizedCertificates: SSLManagedCertificate[] = [];
  const seenIds = new Set<string>();
  for (const certificate of certificates) {
    if (seenIds.has(certificate.id)) continue;
    seenIds.add(certificate.id);
    normalizedCertificates.push(certificate);
  }

  const legacyCert = typeof raw.cert === "string" ? raw.cert.trim() : "";
  const legacyKey = typeof raw.key === "string" ? raw.key.trim() : "";
  let legacyMatch: SSLManagedCertificate | null = null;

  if (legacyCert && legacyKey) {
    legacyMatch = findMatchingSSLCertificate(
      normalizedCertificates,
      legacyCert,
      legacyKey,
    );

    if (!legacyMatch) {
      const migrated = normalizeManagedSSLCertificate({
        id: buildSSLCertificateId(legacyCert, legacyKey),
        label: "当前证书",
        source: "manual",
        cert: legacyCert,
        key: legacyKey,
      });
      if (migrated) {
        normalizedCertificates.unshift(migrated);
        legacyMatch = migrated;
      }
    }
  }

  const activeFromId =
    typeof raw.active_cert_id === "string" && raw.active_cert_id.trim()
      ? normalizedCertificates.find(
          (item) => item.id === raw.active_cert_id?.trim(),
        ) || null
      : null;
  const activeCertificate = activeFromId || legacyMatch || null;

  return {
    cert: activeCertificate?.cert || "",
    key: activeCertificate?.key || "",
    active_cert_id: activeCertificate?.id || "",
    deployment_mode: normalizeSSLDeploymentMode(raw.deployment_mode),
    certificates: normalizedCertificates,
  };
};

const mirrorActiveSSLCertificate = (
  ssl: SSLConfig,
  activeCertId?: string | null,
): SSLConfig => {
  const normalized = normalizeSSLConfig(ssl);
  const active =
    activeCertId && activeCertId.trim()
      ? normalized.certificates?.find((item) => item.id === activeCertId) ||
        null
      : null;

  return {
    ...normalized,
    cert: active?.cert || "",
    key: active?.key || "",
    active_cert_id: active?.id || "",
  };
};

const normalizeHostAccessMode = (value: unknown): HostAccessMode =>
  value === "strict_whitelist" ? "strict_whitelist" : "login_first";

const normalizeHostServiceRole = (value: unknown): HostServiceRole =>
  value === "auth" ? "auth" : "app";

const normalizeStreamProtocol = (value: unknown): StreamMappingProtocol =>
  value === "udp" ? "udp" : "tcp";

const normalizeHostMapping = (
  value?: Partial<HostMapping> | null,
): HostMapping => {
  const raw = value ?? {};
  const target = typeof raw.target === "string" ? raw.target.trim() : "";
  const serviceRole = isAuthServiceTarget(target)
    ? "auth"
    : normalizeHostServiceRole(raw.service_role);

  return {
    host: normalizeHost(raw.host),
    target,
    use_auth: serviceRole === "auth" ? false : raw.use_auth !== false,
    access_mode:
      serviceRole === "auth"
        ? "login_first"
        : normalizeHostAccessMode(raw.access_mode),
    suppress_toolbar:
      serviceRole === "auth" ? false : raw.suppress_toolbar === true,
    preserve_host: raw.preserve_host === true,
    service_role: serviceRole,
    title: typeof raw.title === "string" ? raw.title.trim() : "",
    title_override:
      typeof raw.title_override === "string" ? raw.title_override.trim() : "",
    favicon: typeof raw.favicon === "string" ? raw.favicon.trim() : "",
  };
};

const normalizeHostMappings = (
  value?: Array<Partial<HostMapping>> | null,
): HostMapping[] => {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => normalizeHostMapping(item))
    .filter((item) => item.host && item.target);
};

const normalizeStreamMapping = (
  value?: Partial<StreamMapping> | null,
): StreamMapping => {
  const raw = value ?? {};

  return {
    protocol: normalizeStreamProtocol(raw.protocol),
    listen_port: normalizePositiveInt(raw.listen_port, 0, {
      min: 1,
      max: 65535,
    }),
    target: typeof raw.target === "string" ? raw.target.trim() : "",
    use_auth: raw.use_auth !== false,
  };
};

const normalizeStreamMappings = (
  value?: Array<Partial<StreamMapping>> | null,
): StreamMapping[] => {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => normalizeStreamMapping(item))
    .filter((item) => item.listen_port > 0 && item.target);
};

const normalizeSubdomainModeConfig = (
  value?: Partial<SubdomainModeConfig> | null,
): SubdomainModeConfig => {
  const raw = value ?? {};
  const normalizePublicPort = (input: unknown): number => {
    const port =
      typeof input === "number"
        ? input
        : Number.parseInt(String(input ?? ""), 10);
    if (!Number.isFinite(port) || port <= 0) return 0;
    return Math.floor(port);
  };
  const normalizeCacheTTL = (input: unknown, fallback: number): number => {
    const ttl =
      typeof input === "number"
        ? input
        : Number.parseInt(String(input ?? ""), 10);
    if (!Number.isFinite(ttl) || ttl < 0) return fallback;
    return Math.floor(ttl);
  };

  return {
    root_domain:
      typeof raw.root_domain === "string"
        ? raw.root_domain.trim().toLowerCase()
        : "",
    auth_host: normalizeHost(raw.auth_host),
    auth_target:
      typeof raw.auth_target === "string" && raw.auth_target.trim()
        ? raw.auth_target.trim()
        : DEFAULT_CONFIG.subdomain_mode.auth_target,
    cookie_domain:
      typeof raw.cookie_domain === "string" ? raw.cookie_domain.trim() : "",
    public_auth_base_url:
      typeof raw.public_auth_base_url === "string"
        ? raw.public_auth_base_url.trim().replace(/\/+$/, "")
        : "",
    public_http_port: normalizePublicPort(raw.public_http_port),
    public_https_port: normalizePublicPort(raw.public_https_port),
    auth_cache_ttl_seconds: normalizeCacheTTL(
      raw.auth_cache_ttl_seconds,
      DEFAULT_CONFIG.subdomain_mode.auth_cache_ttl_seconds,
    ),
    auth_cache_unauthorized_ttl_seconds: normalizeCacheTTL(
      raw.auth_cache_unauthorized_ttl_seconds,
      DEFAULT_CONFIG.subdomain_mode.auth_cache_unauthorized_ttl_seconds,
    ),
    default_access_mode: normalizeHostAccessMode(raw.default_access_mode),
    auto_add_whitelist_on_login: raw.auto_add_whitelist_on_login !== false,
    passkey_rp_mode:
      raw.passkey_rp_mode === "parent_domain" ? "parent_domain" : "auth_host",
    passkey_rp_id:
      typeof raw.passkey_rp_id === "string"
        ? raw.passkey_rp_id.trim().toLowerCase()
        : "",
  };
};

const normalizeCaptchaSettings = (
  value?: Partial<CaptchaSettings> | null,
): CaptchaSettings => {
  const raw = value ?? {};
  const provider = raw.provider === "turnstile" ? "turnstile" : "pow";
  const turnstileRaw: Partial<TurnstileCaptchaConfig> = raw.turnstile ?? {};

  return {
    provider,
    widget_mode: "normal",
    pow: {},
    turnstile: {
      site_key:
        typeof turnstileRaw.site_key === "string"
          ? turnstileRaw.site_key.trim()
          : "",
      secret_key:
        typeof turnstileRaw.secret_key === "string"
          ? turnstileRaw.secret_key.trim()
          : "",
    },
  };
};

export class ConfigManager {
  private redis: Redis;
  private configKey = "fn_knock:config";
  private captchaSettingsKey = "fn_knock:captcha:settings";
  private protocolMappingFeatureKey = "fn_knock:protocol-mapping:feature";
  private caHostsKey = "fn_knock:ca:hosts";
  private acmeJobKey = "fn_knock:acme:job:";
  private acmeLogsKey = "fn_knock:acme:logs:";
  private acmeCertKey = "fn_knock:acme:cert:";
  private acmeApplicationsKey = "fn_knock:acme:applications";
  private acmeIssuedCertificatesKey = "fn_knock:acme:issued-certificates";
  private acmeRuntimeLockKey = "fn_knock:acme:runtime-lock";
  private acmeMigrationVersionKey = "fn_knock:acme:migration:v1";
  private acmeSettingsKey = "fn_knock:acme:settings";
  private acmeClientSettingsKey = "fn_knock:acme:client-settings";
  private onboardingCompletedKey = "fn_knock:onboarding:completed";
  private runModePromptPreferencesKey = "fn_knock:run-mode:prompt-preferences";
  private reverseProxyThrottlePatchFlagKey =
    LEGACY_REVERSE_PROXY_THROTTLE_PATCH_FLAG_KEY;

  constructor() {
    this.redis = redis;
  }

  getAcmeRuntimeLockTtlSeconds(): number {
    return ACME_RUNTIME_LOCK_TTL_SECONDS;
  }

  private buildAcmeRuntimeLockLease(
    lock: AcmeRuntimeLock,
    ttlSeconds: number = this.getAcmeRuntimeLockTtlSeconds(),
  ): AcmeRuntimeLock {
    const now = new Date();
    const next = normalizeAcmeRuntimeLock({
      ...lock,
      locked: true,
      lockId:
        normalizeOptionalString(lock.lockId) || randomBytes(16).toString("hex"),
      heartbeatAt: now.toISOString(),
      expiresAt: new Date(now.getTime() + ttlSeconds * 1000).toISOString(),
    });

    return {
      ...next,
      locked: true,
    };
  }

  private async readAcmeRuntimeLockRecord(): Promise<{
    lock: AcmeRuntimeLock;
    raw: string | null;
    ttlMs: number;
  }> {
    const [raw, ttlMs] = await Promise.all([
      this.redis.get(this.acmeRuntimeLockKey),
      this.redis.pttl(this.acmeRuntimeLockKey),
    ]);

    if (!raw) {
      return { lock: { locked: false }, raw: null, ttlMs: -2 };
    }

    try {
      return {
        lock: normalizeAcmeRuntimeLock(JSON.parse(raw)),
        raw,
        ttlMs,
      };
    } catch {
      return {
        lock: { locked: false },
        raw,
        ttlMs,
      };
    }
  }

  private async clearAcmeRuntimeLockIfRawMatches(
    expectedRaw: string,
  ): Promise<boolean> {
    const result = await this.redis.eval(
      `
        local raw = redis.call("GET", KEYS[1])
        if not raw or raw ~= ARGV[1] then
          return 0
        end
        redis.call("DEL", KEYS[1])
        return 1
      `,
      1,
      this.acmeRuntimeLockKey,
      expectedRaw,
    );

    return result === 1;
  }

  private async updateAcmeRuntimeLockLeaseIfOwned(
    lockId: string,
    next: AcmeRuntimeLock,
    ttlSeconds: number,
  ): Promise<boolean> {
    const result = await this.redis.eval(
      `
        local raw = redis.call("GET", KEYS[1])
        if not raw then
          return 0
        end
        local ok, decoded = pcall(cjson.decode, raw)
        if not ok or type(decoded) ~= "table" or decoded["lockId"] ~= ARGV[1] then
          return 0
        end
        redis.call("SET", KEYS[1], ARGV[2], "EX", tonumber(ARGV[3]))
        return 1
      `,
      1,
      this.acmeRuntimeLockKey,
      lockId,
      JSON.stringify(next),
      String(ttlSeconds),
    );

    return result === 1;
  }

  private async clearAcmeRuntimeLockIfOwned(lockId: string): Promise<boolean> {
    const result = await this.redis.eval(
      `
        local raw = redis.call("GET", KEYS[1])
        if not raw then
          return 0
        end
        local ok, decoded = pcall(cjson.decode, raw)
        if not ok or type(decoded) ~= "table" or decoded["lockId"] ~= ARGV[1] then
          return 0
        end
        redis.call("DEL", KEYS[1])
        return 1
      `,
      1,
      this.acmeRuntimeLockKey,
      lockId,
    );

    return result === 1;
  }

  private isAcmeRuntimeLockExpired(
    lock: AcmeRuntimeLock,
    ttlMs: number,
  ): boolean {
    if (!lock.locked) return false;
    if (ttlMs >= 0) return false;

    const expiresAtMs = Date.parse(lock.expiresAt || "");
    if (Number.isFinite(expiresAtMs)) {
      return expiresAtMs <= Date.now();
    }

    const startedAtMs = Date.parse(lock.heartbeatAt || lock.startedAt || "");
    if (!Number.isFinite(startedAtMs)) {
      return true;
    }

    return (
      startedAtMs + this.getAcmeRuntimeLockTtlSeconds() * 1000 <= Date.now()
    );
  }

  isAcmeIssuedCertificateCompatible(
    application:
      | Pick<AcmeApplication, "domains" | "primaryDomain">
      | null
      | undefined,
    issuedCertificate:
      | Pick<AcmeIssuedCertificate, "primaryDomain" | "certInfo">
      | null
      | undefined,
  ): boolean {
    if (!application || !issuedCertificate) return false;
    if (issuedCertificate.primaryDomain !== application.primaryDomain) {
      return false;
    }
    return hasSameNormalizedDomainSet(
      application.domains,
      issuedCertificate.certInfo.dnsNames,
    );
  }

  async getUsableAcmeIssuedCertificate(
    applicationId: string,
  ): Promise<AcmeIssuedCertificate | null> {
    const [application, issuedCertificate] = await Promise.all([
      this.getAcmeApplication(applicationId),
      this.getAcmeIssuedCertificate(applicationId),
    ]);
    if (
      !this.isAcmeIssuedCertificateCompatible(application, issuedCertificate)
    ) {
      return null;
    }
    return issuedCertificate;
  }

  async applyLegacyReverseProxyThrottlePatchIfNeeded(): Promise<{
    applied: boolean;
    config: AppConfig;
  }> {
    const [config, patchFlag] = await Promise.all([
      this.getConfig(),
      this.redis.get(this.reverseProxyThrottlePatchFlagKey),
    ]);

    if (patchFlag === "1") {
      return { applied: false, config };
    }

    const currentThrottle = normalizeReverseProxyThrottleConfig(
      config.reverse_proxy_throttle,
    );
    const shouldPatch =
      currentThrottle.requests_per_second ===
        LEGACY_REVERSE_PROXY_THROTTLE_CONFIG.requests_per_second &&
      currentThrottle.burst === LEGACY_REVERSE_PROXY_THROTTLE_CONFIG.burst &&
      currentThrottle.block_seconds ===
        LEGACY_REVERSE_PROXY_THROTTLE_CONFIG.block_seconds;

    if (!shouldPatch) {
      await this.redis.set(this.reverseProxyThrottlePatchFlagKey, "1");
      return { applied: false, config };
    }

    const nextConfig: AppConfig = {
      ...config,
      reverse_proxy_throttle: {
        ...currentThrottle,
        requests_per_second:
          DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG.requests_per_second,
        burst: DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG.burst,
        block_seconds: DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG.block_seconds,
      },
    };

    await this.redis
      .multi()
      .set(this.configKey, JSON.stringify(nextConfig))
      .set(this.reverseProxyThrottlePatchFlagKey, "1")
      .exec();

    return {
      applied: true,
      config: nextConfig,
    };
  }

  async getConfig(): Promise<AppConfig> {
    try {
      const data = await this.redis.get(this.configKey);
      if (data) {
        // 处理已有数据缺少 default_route 的兼容情况
        const parsed = JSON.parse(data) as AppConfig;
        if (![0, 1, 3].includes(parsed.run_type)) parsed.run_type = 1;
        parsed.reverse_proxy_submode = normalizeReverseProxySubmode(
          parsed.reverse_proxy_submode,
        );
        if (!parsed.default_route) parsed.default_route = "/__select__";
        if (!parsed.default_tunnel) parsed.default_tunnel = "frp";
        parsed.host_mappings = normalizeHostMappings(parsed.host_mappings);
        parsed.stream_mappings = normalizeStreamMappings(
          parsed.stream_mappings,
        );
        parsed.subdomain_mode = normalizeSubdomainModeConfig(
          parsed.subdomain_mode,
        );
        parsed.ssl = normalizeSSLConfig(parsed.ssl);
        parsed.fnos_share_bypass = normalizeFnosShareBypassConfig(
          parsed.fnos_share_bypass,
        );
        parsed.gateway_logging = normalizeGatewayLoggingSettings(
          parsed.gateway_logging,
        );
        parsed.reverse_proxy_throttle = normalizeReverseProxyThrottleConfig(
          parsed.reverse_proxy_throttle,
        );
        parsed.auth_credential_settings = normalizeAuthCredentialSettings(
          parsed.auth_credential_settings,
          {
            legacyAutoAddWhitelistOnLogin:
              parsed.subdomain_mode?.auto_add_whitelist_on_login,
          },
        );
        parsed.terminal_feature = normalizeTerminalFeatureConfig(
          parsed.terminal_feature,
        );
        return parsed;
      }
    } catch (e) {
      console.error("Failed to parse config from redis", e);
    }
    return {
      ...DEFAULT_CONFIG,
      host_mappings: [],
      stream_mappings: [],
      subdomain_mode: { ...DEFAULT_CONFIG.subdomain_mode },
      ssl: normalizeSSLConfig(DEFAULT_CONFIG.ssl),
      fnos_share_bypass: { ...DEFAULT_FNOS_SHARE_BYPASS_CONFIG },
      gateway_logging: { ...DEFAULT_GATEWAY_LOGGING_SETTINGS },
      reverse_proxy_throttle: { ...DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG },
      auth_credential_settings: { ...DEFAULT_AUTH_CREDENTIAL_SETTINGS },
      terminal_feature: { ...DEFAULT_TERMINAL_FEATURE_CONFIG },
    };
  }

  /**
   * 返回不含 SSL cert/key 原文的配置（供 /api/admin/config 使用）
   */
  async getConfigSafe(): Promise<any> {
    const [config, protocolMappingFeature] = await Promise.all([
      this.getConfig(),
      this.getProtocolMappingFeatureConfig(),
    ]);
    const { ssl, ...rest } = config;
    return {
      ...rest,
      protocol_mapping_feature: protocolMappingFeature,
      ssl: {
        enabled: !!(ssl.cert && ssl.key),
        active_cert_id: ssl.active_cert_id || undefined,
        deployment_mode: ssl.deployment_mode || "single_active",
        certificate_count: ssl.certificates?.length || 0,
      },
      terminal_feature: normalizeTerminalFeatureConfig(config.terminal_feature),
    };
  }

  /**
   * 解析 X.509 证书，返回结构化信息
   */
  private parseCertInfo(certPem: string): SSLCertInfo | null {
    try {
      const x509 = new X509Certificate(certPem);
      // 解析 DNS Names (以及 IP) from subjectAltName
      const sanStr = x509.subjectAltName || "";
      const dnsNames: string[] = [];
      sanStr
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean)
        .forEach((entry) => {
          const idx = entry.indexOf(":");
          if (idx <= 0) return;
          const label = entry.slice(0, idx).trim().toLowerCase();
          const value = entry.slice(idx + 1).trim();
          // 常见标签：DNS、IP、IP Address（Node/openssl 输出可能不同）
          if (label === "dns" || label === "ip" || label === "ip address") {
            dnsNames.push(value);
          }
        });
      const subjectCommonName =
        x509.subject
          .split("\n")
          .map((entry) => entry.trim())
          .find((entry) => /^CN\s*=/.test(entry))
          ?.replace(/^CN\s*=\s*/i, "")
          .trim() || "";
      if (
        subjectCommonName &&
        !dnsNames.some(
          (entry) => entry.toLowerCase() === subjectCommonName.toLowerCase(),
        )
      ) {
        dnsNames.push(subjectCommonName);
      }

      return {
        issuer: x509.issuer,
        subject: x509.subject,
        validFrom: x509.validFrom,
        validTo: x509.validTo,
        dnsNames,
        serialNumber: x509.serialNumber,
      };
    } catch (e) {
      console.error("Failed to parse X.509 certificate:", e);
      return null;
    }
  }

  /**
   * 获取 SSL 状态和证书结构化信息
   */
  async getSSLStatus(): Promise<SSLStatus> {
    const config = await this.getConfig();
    const ssl = normalizeSSLConfig(config.ssl);
    const activeCertId = ssl.active_cert_id?.trim() || "";
    const certificates = (ssl.certificates || []).map((item) => {
      const certInfo = this.parseCertInfo(item.cert);
      return {
        id: item.id,
        label: item.label,
        source: item.source,
        primary_domain: item.primary_domain,
        source_ref_id: item.source_ref_id,
        created_at: item.created_at,
        updated_at: item.updated_at,
        certInfo: certInfo || undefined,
        is_active: item.id === activeCertId,
      };
    });
    const activeCertificate =
      certificates.find((item) => item.is_active) || null;
    const certInfo = activeCertificate?.certInfo;

    return {
      enabled: !!activeCertificate,
      activeCertId: activeCertificate?.id,
      deploymentMode: ssl.deployment_mode || "single_active",
      certInfo: certInfo || undefined,
      certificates,
    };
  }

  /**
   * 验证 SSL 证书和私钥是否合法
   */
  validateSSLCert(
    cert: string,
    key: string,
  ): { valid: boolean; error?: string } {
    try {
      new X509Certificate(cert);
    } catch (e: any) {
      return { valid: false, error: `证书格式无效: ${e.message}` };
    }
    try {
      createPrivateKey(key);
    } catch (e: any) {
      return { valid: false, error: `私钥格式无效: ${e.message}` };
    }
    // Check cert-key match
    try {
      const x509 = new X509Certificate(cert);
      const privateKey = createPrivateKey(key);
      if (!x509.checkPrivateKey(privateKey)) {
        return { valid: false, error: "证书与私钥不匹配" };
      }
    } catch (e: any) {
      return { valid: false, error: `证书与私钥校验失败: ${e.message}` };
    }
    return { valid: true };
  }

  /**
   * 清除 SSL 配置
   */
  async clearSSL(): Promise<void> {
    const config = await this.getConfig();
    config.ssl = mirrorActiveSSLCertificate(config.ssl, null);
    await this.saveConfig(config);
  }

  async clearSSLCertificateLibrary(): Promise<number> {
    const config = await this.getConfig();
    const removedCount = config.ssl.certificates?.length || 0;
    config.ssl = {
      ...config.ssl,
      certificates: [],
    };
    config.ssl = mirrorActiveSSLCertificate(config.ssl, null);
    await this.saveConfig(config);
    return removedCount;
  }

  async saveConfig(config: AppConfig): Promise<void> {
    await this.redis.set(this.configKey, JSON.stringify(config));
  }

  async getSSLCertificate(id: string): Promise<SSLManagedCertificate | null> {
    const config = await this.getConfig();
    return (
      config.ssl.certificates?.find((certificate) => certificate.id === id) ||
      null
    );
  }

  async getActiveSSLCertificate(): Promise<SSLManagedCertificate | null> {
    const config = await this.getConfig();
    const activeId = config.ssl.active_cert_id?.trim();
    if (!activeId) return null;
    return (
      config.ssl.certificates?.find(
        (certificate) => certificate.id === activeId,
      ) || null
    );
  }

  async saveSSLCertificate(input: {
    id?: string;
    label?: string;
    source?: SSLCertificateSource;
    primary_domain?: string;
    source_ref_id?: string;
    cert: string;
    key: string;
    activate?: boolean;
    matchBy?: {
      source?: SSLCertificateSource;
      primary_domain?: string;
      source_ref_id?: string;
      cert?: string;
      key?: string;
    };
  }): Promise<SSLManagedCertificate> {
    const config = await this.getConfig();
    const ssl = normalizeSSLConfig(config.ssl);
    const certificates = [...(ssl.certificates || [])];
    const now = new Date().toISOString();

    let existing =
      (input.id
        ? certificates.find((certificate) => certificate.id === input.id)
        : undefined) || null;

    if (
      !existing &&
      input.matchBy?.source &&
      input.matchBy?.source_ref_id?.trim()
    ) {
      existing =
        certificates.find(
          (certificate) =>
            certificate.source === input.matchBy?.source &&
            certificate.source_ref_id === input.matchBy?.source_ref_id?.trim(),
        ) || null;
    }

    if (
      !existing &&
      input.matchBy?.source &&
      input.matchBy?.primary_domain?.trim()
    ) {
      existing =
        certificates.find(
          (certificate) =>
            certificate.source === input.matchBy?.source &&
            certificate.primary_domain ===
              input.matchBy?.primary_domain?.trim().toLowerCase(),
        ) || null;
    }

    if (!existing && input.matchBy?.cert && input.matchBy?.key) {
      existing = findMatchingSSLCertificate(
        certificates,
        input.matchBy.cert.trim(),
        input.matchBy.key.trim(),
      );
    }

    const nextRecord = normalizeManagedSSLCertificate({
      id: existing?.id || input.id,
      label: input.label || existing?.label,
      source: input.source || existing?.source || "manual",
      primary_domain: input.primary_domain || existing?.primary_domain,
      source_ref_id: input.source_ref_id || existing?.source_ref_id,
      cert: input.cert,
      key: input.key,
      created_at: existing?.created_at || now,
      updated_at: now,
    });

    if (!nextRecord) {
      throw new Error("证书内容不能为空");
    }

    const nextCertificates = certificates.filter(
      (certificate) => certificate.id !== nextRecord.id,
    );
    nextCertificates.unshift(nextRecord);

    config.ssl = {
      ...ssl,
      certificates: nextCertificates,
    };
    config.ssl = mirrorActiveSSLCertificate(
      config.ssl,
      input.activate === true ? nextRecord.id : ssl.active_cert_id,
    );
    await this.saveConfig(config);
    return nextRecord;
  }

  async saveAcmeCertificateToLibrary(
    domain: string,
    opts?: {
      id?: string;
      label?: string;
      activate?: boolean;
    },
  ): Promise<SSLManagedCertificate> {
    const normalizedDomain = domain.trim().toLowerCase();
    if (!normalizedDomain) {
      throw new Error("域名不能为空");
    }

    const application =
      await this.getAcmeApplicationByPrimaryDomain(normalizedDomain);
    if (application) {
      return this.saveAcmeCertificateToLibraryByApplication(
        application.id,
        opts,
      );
    }

    const pair = await this.getAcmeCert(normalizedDomain);
    if (!pair) {
      throw new Error("证书不存在");
    }

    const validation = this.validateSSLCert(pair.cert, pair.key);
    if (!validation.valid) {
      throw new Error(validation.error || "证书或私钥无效");
    }

    return this.saveSSLCertificate({
      id: opts?.id,
      label: opts?.label || normalizedDomain,
      source: "acme",
      primary_domain: normalizedDomain,
      cert: pair.cert,
      key: pair.key,
      activate: opts?.activate === true,
      matchBy: {
        source: "acme",
        primary_domain: normalizedDomain,
      },
    });
  }

  async getSSLCertificateBySourceRef(
    source: SSLCertificateSource,
    sourceRefId: string,
  ): Promise<SSLManagedCertificate | null> {
    const normalizedSourceRefId = sourceRefId.trim();
    if (!normalizedSourceRefId) return null;
    const config = await this.getConfig();
    return (
      config.ssl.certificates?.find(
        (certificate) =>
          certificate.source === source &&
          certificate.source_ref_id === normalizedSourceRefId,
      ) || null
    );
  }

  async activateSSLCertificate(
    id: string | null | undefined,
  ): Promise<SSLManagedCertificate | null> {
    const config = await this.getConfig();
    const normalizedId = typeof id === "string" ? id.trim() : "";
    const active = normalizedId
      ? config.ssl.certificates?.find(
          (certificate) => certificate.id === normalizedId,
        )
      : null;

    config.ssl = mirrorActiveSSLCertificate(config.ssl, active?.id || null);
    await this.saveConfig(config);
    return active || null;
  }

  async deleteSSLCertificate(id: string): Promise<{
    removed: SSLManagedCertificate | null;
    removedActive: boolean;
  }> {
    const config = await this.getConfig();
    const certificates = [...(config.ssl.certificates || [])];
    const removed =
      certificates.find((certificate) => certificate.id === id) || null;
    if (!removed) {
      return { removed: null, removedActive: false };
    }

    const removedActive = config.ssl.active_cert_id === removed.id;
    config.ssl = {
      ...config.ssl,
      certificates: certificates.filter((certificate) => certificate.id !== id),
    };
    config.ssl = mirrorActiveSSLCertificate(
      config.ssl,
      removedActive ? null : config.ssl.active_cert_id,
    );
    await this.saveConfig(config);
    return { removed, removedActive };
  }

  async deleteSSLCertificatesBySource(
    source: SSLCertificateSource,
    primaryDomain?: string,
  ): Promise<{
    removed: SSLManagedCertificate[];
    removedActive: boolean;
  }> {
    const config = await this.getConfig();
    const normalizedPrimaryDomain = primaryDomain?.trim().toLowerCase() || "";
    const removed = (config.ssl.certificates || []).filter((certificate) => {
      if (certificate.source !== source) return false;
      if (!normalizedPrimaryDomain) return true;
      return certificate.primary_domain === normalizedPrimaryDomain;
    });

    if (removed.length === 0) {
      return { removed: [], removedActive: false };
    }

    const removedIds = new Set(removed.map((certificate) => certificate.id));
    const removedActive = removedIds.has(config.ssl.active_cert_id || "");
    config.ssl = {
      ...config.ssl,
      certificates: (config.ssl.certificates || []).filter(
        (certificate) => !removedIds.has(certificate.id),
      ),
    };
    config.ssl = mirrorActiveSSLCertificate(
      config.ssl,
      removedActive ? null : config.ssl.active_cert_id,
    );
    await this.saveConfig(config);
    return { removed, removedActive };
  }

  async deleteSSLCertificatesBySourceRef(
    source: SSLCertificateSource,
    sourceRefId: string,
  ): Promise<{
    removed: SSLManagedCertificate[];
    removedActive: boolean;
  }> {
    const normalizedSourceRefId = sourceRefId.trim();
    if (!normalizedSourceRefId) {
      return { removed: [], removedActive: false };
    }

    const config = await this.getConfig();
    const removed = (config.ssl.certificates || []).filter(
      (certificate) =>
        certificate.source === source &&
        certificate.source_ref_id === normalizedSourceRefId,
    );

    if (removed.length === 0) {
      return { removed: [], removedActive: false };
    }

    const removedIds = new Set(removed.map((certificate) => certificate.id));
    const removedActive = removedIds.has(config.ssl.active_cert_id || "");
    config.ssl = {
      ...config.ssl,
      certificates: (config.ssl.certificates || []).filter(
        (certificate) => !removedIds.has(certificate.id),
      ),
    };
    config.ssl = mirrorActiveSSLCertificate(
      config.ssl,
      removedActive ? null : config.ssl.active_cert_id,
    );
    await this.saveConfig(config);
    return { removed, removedActive };
  }

  private buildAcmeApplicationId(seed?: string): string {
    const normalizedSeed = seed?.trim().toLowerCase();
    if (normalizedSeed) {
      return `acme_app_${createHash("sha256")
        .update(normalizedSeed)
        .digest("hex")
        .slice(0, 16)}`;
    }

    return `acme_app_${randomBytes(8).toString("hex")}`;
  }

  private async readAcmeApplicationsStore(): Promise<AcmeApplication[]> {
    const raw = await this.redis.get(this.acmeApplicationsKey);
    if (!raw) return [];
    try {
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed
        .map((item) => normalizeAcmeApplication(item))
        .filter((item): item is AcmeApplication => item !== null);
    } catch {
      return [];
    }
  }

  private async writeAcmeApplicationsStore(
    applications: AcmeApplication[],
  ): Promise<void> {
    await this.redis.set(
      this.acmeApplicationsKey,
      JSON.stringify(applications),
    );
  }

  private async readAcmeIssuedCertificatesStore(): Promise<
    AcmeIssuedCertificate[]
  > {
    const raw = await this.redis.get(this.acmeIssuedCertificatesKey);
    if (!raw) return [];
    try {
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed
        .map((item) => normalizeAcmeIssuedCertificate(item))
        .filter((item): item is AcmeIssuedCertificate => item !== null);
    } catch {
      return [];
    }
  }

  private async writeAcmeIssuedCertificatesStore(
    issuedCertificates: AcmeIssuedCertificate[],
  ): Promise<void> {
    await this.redis.set(
      this.acmeIssuedCertificatesKey,
      JSON.stringify(issuedCertificates),
    );
  }

  private async reconcileAcmeLibraryLinks(
    applications: AcmeApplication[],
    issuedCertificates: AcmeIssuedCertificate[],
  ): Promise<AcmeIssuedCertificate[]> {
    if (!applications.length) return issuedCertificates;

    const config = await this.getConfig();
    const certificates = [...(config.ssl.certificates || [])];
    const nextIssuedCertificates = issuedCertificates.map((item) => ({
      ...item,
    }));
    let configChanged = false;
    let issuedChanged = false;

    for (const application of applications) {
      const certificateIndex = certificates.findIndex((certificate) => {
        if (certificate.source !== "acme") return false;
        if (certificate.source_ref_id === application.id) return true;
        return (
          !certificate.source_ref_id &&
          certificate.primary_domain === application.primaryDomain
        );
      });

      if (certificateIndex === -1) continue;

      const libraryCertificate = certificates[certificateIndex]!;
      if (libraryCertificate.source_ref_id !== application.id) {
        certificates[certificateIndex] = {
          ...libraryCertificate,
          source_ref_id: application.id,
        };
        configChanged = true;
      }

      const issuedIndex = nextIssuedCertificates.findIndex(
        (item) => item.applicationId === application.id,
      );
      if (issuedIndex === -1) continue;

      const issuedCertificate = nextIssuedCertificates[issuedIndex]!;
      if (
        issuedCertificate.libraryCertificateId !== libraryCertificate.id ||
        !issuedCertificate.libraryLinkedAt
      ) {
        nextIssuedCertificates[issuedIndex] = {
          ...issuedCertificate,
          libraryCertificateId: libraryCertificate.id,
          libraryLinkedAt:
            issuedCertificate.libraryLinkedAt || libraryCertificate.updated_at,
        };
        issuedChanged = true;
      }
    }

    for (const [index, issuedCertificate] of nextIssuedCertificates.entries()) {
      if (!issuedCertificate.libraryCertificateId) continue;
      const stillExists = certificates.some(
        (certificate) =>
          certificate.source === "acme" &&
          certificate.id === issuedCertificate.libraryCertificateId,
      );
      if (stillExists) continue;
      nextIssuedCertificates[index] = {
        ...issuedCertificate,
        libraryCertificateId: undefined,
        libraryLinkedAt: undefined,
      };
      issuedChanged = true;
    }

    if (configChanged) {
      config.ssl = {
        ...config.ssl,
        certificates,
      };
      config.ssl = mirrorActiveSSLCertificate(
        config.ssl,
        config.ssl.active_cert_id,
      );
      await this.saveConfig(config);
    }

    if (issuedChanged) {
      await this.writeAcmeIssuedCertificatesStore(nextIssuedCertificates);
    }

    return nextIssuedCertificates;
  }

  async ensureAcmeDataMigrated(): Promise<void> {
    const migrationVersion = await this.redis.get(this.acmeMigrationVersionKey);
    const existingApplications = await this.readAcmeApplicationsStore();

    if (existingApplications.length > 0) {
      const issuedCertificates = await this.readAcmeIssuedCertificatesStore();
      await this.reconcileAcmeLibraryLinks(
        existingApplications,
        issuedCertificates,
      );
      if (migrationVersion !== "1") {
        await this.redis.set(this.acmeMigrationVersionKey, "1");
      }
      return;
    }

    const legacySettings = await this.getAcmeSettingsLegacy();
    if (!legacySettings?.domains?.length) {
      if (migrationVersion !== "1") {
        await this.redis.set(this.acmeMigrationVersionKey, "1");
      }
      return;
    }

    const now = new Date().toISOString();
    const primaryDomain = legacySettings.domains[0]!;
    const application: AcmeApplication = {
      id: this.buildAcmeApplicationId(primaryDomain),
      domains: legacySettings.domains,
      primaryDomain,
      dnsType: legacySettings.dnsType,
      credentials: { ...legacySettings.credentials },
      renewEnabled: true,
      createdAt: legacySettings.updatedAt || now,
      updatedAt: legacySettings.updatedAt || now,
      latestJobStatus: "idle",
    };

    const issuedCertificates: AcmeIssuedCertificate[] = [];
    const pair = await this.getAcmeCert(primaryDomain);
    if (pair) {
      const certInfo = this.parseCertInfo(pair.cert);
      if (certInfo) {
        issuedCertificates.push({
          applicationId: application.id,
          primaryDomain,
          cert: pair.cert,
          key: pair.key,
          certInfo,
          createdAt: now,
          updatedAt: now,
        });
      }
    }

    await this.writeAcmeApplicationsStore([application]);
    await this.writeAcmeIssuedCertificatesStore(issuedCertificates);
    await this.reconcileAcmeLibraryLinks([application], issuedCertificates);
    await this.redis.set(this.acmeMigrationVersionKey, "1");
  }

  async listAcmeApplications(): Promise<AcmeApplication[]> {
    await this.ensureAcmeDataMigrated();
    const applications = await this.readAcmeApplicationsStore();
    return applications.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  }

  async getAcmeApplication(id: string): Promise<AcmeApplication | null> {
    await this.ensureAcmeDataMigrated();
    const applications = await this.readAcmeApplicationsStore();
    return applications.find((item) => item.id === id) || null;
  }

  async getAcmeApplicationByPrimaryDomain(
    primaryDomain: string,
  ): Promise<AcmeApplication | null> {
    await this.ensureAcmeDataMigrated();
    const normalizedPrimaryDomain = primaryDomain.trim().toLowerCase();
    if (!normalizedPrimaryDomain) return null;
    const applications = await this.readAcmeApplicationsStore();
    return (
      applications.find(
        (item) => item.primaryDomain === normalizedPrimaryDomain,
      ) || null
    );
  }

  async saveAcmeApplication(input: {
    id?: string;
    name?: string;
    domains: string[];
    dnsType: string;
    credentials: Record<string, string>;
    renewEnabled?: boolean;
  }): Promise<AcmeApplication> {
    const result = await this.saveAcmeApplicationWithEffects(input);
    return result.application;
  }

  async saveAcmeApplicationWithEffects(input: {
    id?: string;
    name?: string;
    domains: string[];
    dnsType: string;
    credentials: Record<string, string>;
    renewEnabled?: boolean;
  }): Promise<AcmeApplicationSaveResult> {
    await this.ensureAcmeDataMigrated();
    const applications = await this.readAcmeApplicationsStore();
    const normalizedDomains = normalizeDomainList(input.domains);
    const primaryDomain = normalizedDomains[0] || "";
    const dnsType = input.dnsType.trim();

    if (!normalizedDomains.length) {
      throw new Error("域名列表不能为空");
    }
    if (!dnsType) {
      throw new Error("DNS 服务商不能为空");
    }

    const existing = input.id
      ? applications.find((item) => item.id === input.id) || null
      : null;
    const duplicated = applications.find(
      (item) =>
        item.primaryDomain === primaryDomain && item.id !== existing?.id,
    );
    if (duplicated) {
      throw new Error(`主域名 ${primaryDomain} 已存在于其他申请项中`);
    }

    const now = new Date().toISOString();
    const next: AcmeApplication = {
      id: existing?.id || this.buildAcmeApplicationId(),
      name:
        input.name !== undefined
          ? normalizeOptionalString(input.name)
          : existing?.name,
      domains: normalizedDomains,
      primaryDomain,
      dnsType,
      credentials: normalizeStringRecord(input.credentials),
      renewEnabled: input.renewEnabled ?? existing?.renewEnabled ?? true,
      createdAt: existing?.createdAt || now,
      updatedAt: now,
      latestJobId: existing?.latestJobId,
      latestJobStatus: existing?.latestJobStatus || "idle",
      latestJobTrigger: existing?.latestJobTrigger,
      latestJobAt: existing?.latestJobAt,
      lastError: existing?.lastError,
    };

    const nextApplications = applications.filter((item) => item.id !== next.id);
    nextApplications.unshift(next);
    await this.writeAcmeApplicationsStore(nextApplications);

    const domainChanged =
      !!existing &&
      (!hasSameNormalizedDomainSet(existing.domains, next.domains) ||
        existing.primaryDomain !== next.primaryDomain);

    if (!domainChanged) {
      return {
        application: next,
        certificateInvalidated: false,
        deletedIssuedCertificate: null,
        removedLibraryCertificates: [],
        removedActiveLibraryCertificate: false,
        removedDomains: [],
      };
    }

    const deletedIssuedCertificate = await this.deleteAcmeIssuedCertificate(
      next.id,
    );
    const deletedBySourceRef = await this.deleteSSLCertificatesBySourceRef(
      "acme",
      next.id,
    );
    const deletedByPrimaryDomain = await this.deleteSSLCertificatesBySource(
      "acme",
      existing.primaryDomain,
    );
    const removedLibraryCertificates = [
      ...deletedBySourceRef.removed,
      ...deletedByPrimaryDomain.removed.filter(
        (certificate) =>
          !deletedBySourceRef.removed.some(
            (item) => item.id === certificate.id,
          ),
      ),
    ];
    const removedDomains = Array.from(
      new Set(
        [
          existing.primaryDomain,
          deletedIssuedCertificate?.primaryDomain,
        ].filter((value): value is string => Boolean(value)),
      ),
    );

    if (removedDomains.length > 0) {
      const { join } = await import("node:path");
      const { rm } = await import("node:fs/promises");
      for (const domain of removedDomains) {
        await this.deleteAcmeCert(domain);
        await rm(join(process.cwd(), "data", "ssl", domain), {
          recursive: true,
          force: true,
        });
      }
    }

    return {
      application: next,
      certificateInvalidated: true,
      deletedIssuedCertificate,
      removedLibraryCertificates,
      removedActiveLibraryCertificate:
        deletedBySourceRef.removedActive ||
        deletedByPrimaryDomain.removedActive,
      removedDomains,
    };
  }

  async updateAcmeApplicationJobState(
    applicationId: string,
    job: Pick<
      AcmeJob,
      | "id"
      | "status"
      | "trigger"
      | "createdAt"
      | "startedAt"
      | "finishedAt"
      | "message"
    >,
  ): Promise<AcmeApplication | null> {
    await this.ensureAcmeDataMigrated();
    const applications = await this.readAcmeApplicationsStore();
    const index = applications.findIndex((item) => item.id === applicationId);
    if (index === -1) return null;

    const existing = applications[index]!;
    const latestJobAt =
      job.finishedAt ||
      job.startedAt ||
      job.createdAt ||
      new Date().toISOString();

    const next: AcmeApplication = {
      ...existing,
      latestJobId: job.id,
      latestJobStatus: job.status,
      latestJobTrigger: job.trigger,
      latestJobAt,
      lastError:
        job.status === "failed"
          ? normalizeOptionalString(job.message)
          : undefined,
    };

    applications[index] = next;
    await this.writeAcmeApplicationsStore(applications);
    return next;
  }

  async listAcmeIssuedCertificates(): Promise<AcmeIssuedCertificate[]> {
    await this.ensureAcmeDataMigrated();
    return this.readAcmeIssuedCertificatesStore();
  }

  async getAcmeIssuedCertificate(
    applicationId: string,
  ): Promise<AcmeIssuedCertificate | null> {
    await this.ensureAcmeDataMigrated();
    const issuedCertificates = await this.readAcmeIssuedCertificatesStore();
    return (
      issuedCertificates.find((item) => item.applicationId === applicationId) ||
      null
    );
  }

  async getAcmeIssuedCertificateByPrimaryDomain(
    primaryDomain: string,
  ): Promise<AcmeIssuedCertificate | null> {
    await this.ensureAcmeDataMigrated();
    const normalizedPrimaryDomain = primaryDomain.trim().toLowerCase();
    if (!normalizedPrimaryDomain) return null;
    const issuedCertificates = await this.readAcmeIssuedCertificatesStore();
    return (
      issuedCertificates.find(
        (item) => item.primaryDomain === normalizedPrimaryDomain,
      ) || null
    );
  }

  async saveAcmeIssuedCertificate(input: {
    applicationId: string;
    primaryDomain: string;
    cert: string;
    key: string;
    certInfo: SSLCertInfo;
    libraryCertificateId?: string;
  }): Promise<AcmeIssuedCertificate> {
    await this.ensureAcmeDataMigrated();
    const issuedCertificates = await this.readAcmeIssuedCertificatesStore();
    const existing =
      issuedCertificates.find(
        (item) => item.applicationId === input.applicationId,
      ) || null;
    const now = new Date().toISOString();

    const next: AcmeIssuedCertificate = {
      applicationId: input.applicationId.trim(),
      primaryDomain: input.primaryDomain.trim().toLowerCase(),
      cert: input.cert.trim(),
      key: input.key.trim(),
      certInfo: input.certInfo,
      createdAt: existing?.createdAt || now,
      updatedAt: now,
      libraryCertificateId:
        normalizeOptionalString(input.libraryCertificateId) ||
        existing?.libraryCertificateId,
      libraryLinkedAt: normalizeOptionalString(input.libraryCertificateId)
        ? now
        : existing?.libraryLinkedAt,
    };

    const nextIssuedCertificates = issuedCertificates.filter(
      (item) => item.applicationId !== next.applicationId,
    );
    nextIssuedCertificates.unshift(next);
    await this.writeAcmeIssuedCertificatesStore(nextIssuedCertificates);
    await this.saveAcmeCert(next.primaryDomain, next.cert, next.key);
    return next;
  }

  async linkAcmeIssuedCertificateToLibrary(
    applicationId: string,
    libraryCertificateId?: string | null,
  ): Promise<AcmeIssuedCertificate | null> {
    await this.ensureAcmeDataMigrated();
    const issuedCertificates = await this.readAcmeIssuedCertificatesStore();
    const index = issuedCertificates.findIndex(
      (item) => item.applicationId === applicationId,
    );
    if (index === -1) return null;

    const existing = issuedCertificates[index]!;
    issuedCertificates[index] = {
      ...existing,
      updatedAt: new Date().toISOString(),
      libraryCertificateId:
        normalizeOptionalString(libraryCertificateId) || undefined,
      libraryLinkedAt: libraryCertificateId
        ? new Date().toISOString()
        : undefined,
    };
    await this.writeAcmeIssuedCertificatesStore(issuedCertificates);
    return issuedCertificates[index]!;
  }

  async deleteAcmeIssuedCertificate(
    applicationId: string,
  ): Promise<AcmeIssuedCertificate | null> {
    await this.ensureAcmeDataMigrated();
    const issuedCertificates = await this.readAcmeIssuedCertificatesStore();
    const existing =
      issuedCertificates.find((item) => item.applicationId === applicationId) ||
      null;
    if (!existing) return null;

    const nextIssuedCertificates = issuedCertificates.filter(
      (item) => item.applicationId !== applicationId,
    );
    await this.writeAcmeIssuedCertificatesStore(nextIssuedCertificates);
    await this.deleteAcmeCert(existing.primaryDomain);
    return existing;
  }

  async saveAcmeIssuedCertFromFS(
    applicationId: string,
    primaryDomain: string,
    opts?: { forceInstall?: boolean },
  ): Promise<boolean> {
    const saved = await this.saveAcmeCertFromFS(primaryDomain, opts);
    if (!saved) return false;

    const pair = await this.getAcmeCert(primaryDomain);
    if (!pair) return false;

    const certInfo = this.parseCertInfo(pair.cert);
    if (!certInfo) return false;

    const existing = await this.getAcmeIssuedCertificate(applicationId);
    await this.saveAcmeIssuedCertificate({
      applicationId,
      primaryDomain,
      cert: pair.cert,
      key: pair.key,
      certInfo,
      libraryCertificateId: existing?.libraryCertificateId,
    });
    return true;
  }

  async getAcmeRuntimeLock(): Promise<AcmeRuntimeLock> {
    const { lock } = await this.readAcmeRuntimeLockRecord();
    return lock;
  }

  async tryAcquireAcmeRuntimeLock(
    lock: AcmeRuntimeLock,
    ttlSeconds: number = this.getAcmeRuntimeLockTtlSeconds(),
  ): Promise<AcmeRuntimeLock | null> {
    const next = this.buildAcmeRuntimeLockLease(lock, ttlSeconds);
    const result = await this.redis.set(
      this.acmeRuntimeLockKey,
      JSON.stringify(next),
      "EX",
      ttlSeconds,
      "NX",
    );
    return result === "OK" ? next : null;
  }

  async refreshAcmeRuntimeLock(
    lock: AcmeRuntimeLock,
    ttlSeconds: number = this.getAcmeRuntimeLockTtlSeconds(),
  ): Promise<AcmeRuntimeLock | null> {
    const lockId = normalizeOptionalString(lock.lockId);
    if (!lockId) return null;

    const next = this.buildAcmeRuntimeLockLease(lock, ttlSeconds);
    const updated = await this.updateAcmeRuntimeLockLeaseIfOwned(
      lockId,
      next,
      ttlSeconds,
    );
    return updated ? next : null;
  }

  async setAcmeRuntimeLock(lock: AcmeRuntimeLock): Promise<AcmeRuntimeLock> {
    const next = this.buildAcmeRuntimeLockLease(lock);
    await this.redis.set(
      this.acmeRuntimeLockKey,
      JSON.stringify(next),
      "EX",
      this.getAcmeRuntimeLockTtlSeconds(),
    );
    return next;
  }

  async releaseAcmeRuntimeLock(
    lock: AcmeRuntimeLock | string | null | undefined,
  ): Promise<boolean> {
    const lockId =
      typeof lock === "string"
        ? normalizeOptionalString(lock)
        : normalizeOptionalString(lock?.lockId);
    if (lockId) {
      return this.clearAcmeRuntimeLockIfOwned(lockId);
    }
    return false;
  }

  async clearAcmeRuntimeLock(): Promise<void> {
    await this.redis.del(this.acmeRuntimeLockKey);
  }

  async getActiveAcmeRuntimeLock(): Promise<AcmeRuntimeLock> {
    const { lock, raw, ttlMs } = await this.readAcmeRuntimeLockRecord();
    if (!lock.locked || !lock.jobId) {
      if (raw) {
        await this.clearAcmeRuntimeLockIfRawMatches(raw);
      }
      return { locked: false };
    }
    if (this.isAcmeRuntimeLockExpired(lock, ttlMs)) {
      if (lock.lockId) {
        await this.releaseAcmeRuntimeLock(lock.lockId);
      } else if (raw) {
        await this.clearAcmeRuntimeLockIfRawMatches(raw);
      }
      return { locked: false };
    }
    const job = await this.getAcmeJob(lock.jobId);
    if (!job || job.status === "succeeded" || job.status === "failed") {
      if (lock.lockId) {
        await this.releaseAcmeRuntimeLock(lock.lockId);
      } else if (raw) {
        await this.clearAcmeRuntimeLockIfRawMatches(raw);
      }
      return { locked: false };
    }
    return lock;
  }

  async getActiveAcmeJobFromLock(): Promise<AcmeJob | null> {
    const lock = await this.getActiveAcmeRuntimeLock();
    if (!lock.locked || !lock.jobId) return null;
    return this.getAcmeJob(lock.jobId);
  }

  async saveAcmeCertificateToLibraryByApplication(
    applicationId: string,
    opts?: {
      id?: string;
      label?: string;
      activate?: boolean;
    },
  ): Promise<SSLManagedCertificate> {
    const [application, issuedCertificate] = await Promise.all([
      this.getAcmeApplication(applicationId),
      this.getAcmeIssuedCertificate(applicationId),
    ]);
    if (!application) {
      throw new Error("申请项不存在");
    }
    if (
      !this.isAcmeIssuedCertificateCompatible(application, issuedCertificate)
    ) {
      throw new Error("当前申请项还没有与域名配置匹配的已签发证书");
    }
    if (!issuedCertificate) {
      throw new Error("证书不存在");
    }

    const validation = this.validateSSLCert(
      issuedCertificate.cert,
      issuedCertificate.key,
    );
    if (!validation.valid) {
      throw new Error(validation.error || "证书或私钥无效");
    }

    const saved = await this.saveSSLCertificate({
      id: opts?.id || issuedCertificate.libraryCertificateId,
      label: opts?.label || issuedCertificate.primaryDomain,
      source: "acme",
      primary_domain: issuedCertificate.primaryDomain,
      source_ref_id: applicationId,
      cert: issuedCertificate.cert,
      key: issuedCertificate.key,
      activate: opts?.activate === true,
      matchBy: {
        source: "acme",
        source_ref_id: applicationId,
      },
    });

    await this.linkAcmeIssuedCertificateToLibrary(applicationId, saved.id);
    return saved;
  }

  async createAcmeJob(job: AcmeJob): Promise<void> {
    const key = `${this.acmeJobKey}${job.id}`;
    const normalized = normalizeAcmeJob(job);
    if (!normalized) {
      throw new Error("ACME 任务数据无效");
    }
    await this.redis.set(key, JSON.stringify(normalized), "EX", 86400);
  }

  async updateAcmeJob(id: string, patch: Partial<AcmeJob>): Promise<void> {
    const key = `${this.acmeJobKey}${id}`;
    const raw = await this.redis.get(key);
    if (!raw) return;
    let obj: AcmeJob | null = null;
    try {
      obj = normalizeAcmeJob(JSON.parse(raw));
    } catch {
      return;
    }
    if (!obj) return;
    const next = { ...obj, ...patch };
    await this.redis.set(key, JSON.stringify(next), "EX", 86400);
  }

  async getAcmeJob(id: string): Promise<AcmeJob | null> {
    const raw = await this.redis.get(`${this.acmeJobKey}${id}`);
    if (!raw) return null;
    try {
      return normalizeAcmeJob(JSON.parse(raw));
    } catch {
      return null;
    }
  }

  async appendAcmeLog(jobId: string, line: string): Promise<void> {
    const key = `${this.acmeLogsKey}${jobId}`;
    const buffer = new RedisLogBuffer(this.redis, {
      key,
      ttlSeconds: 86400,
      maxLen: DEFAULT_REDIS_LOG_BUFFER_MAX_LEN,
    });
    await buffer.append([line]);
  }

  async clearAcmeLogs(jobId: string): Promise<void> {
    await this.redis.del(`${this.acmeLogsKey}${jobId}`);
  }

  async getAcmeLogs(
    jobId: string,
    limit: number = 500,
    order: "asc" | "desc" = "asc",
  ): Promise<string[]> {
    const key = `${this.acmeLogsKey}${jobId}`;
    const len = await this.redis.llen(key);
    if (len === 0) return [];
    const start = Math.max(0, len - limit);
    const arr = await this.redis.lrange(key, start, -1);
    return order === "desc" ? arr.reverse() : arr;
  }

  private async getAcmeSettingsLegacy(): Promise<AcmeSettings | null> {
    const raw = await this.redis.get(this.acmeSettingsKey);
    if (!raw) return null;
    try {
      const obj = JSON.parse(raw);
      if (!obj || typeof obj !== "object") return null;
      if (
        !Array.isArray(obj.domains) ||
        typeof obj.dnsType !== "string" ||
        typeof obj.credentials !== "object"
      )
        return null;
      return {
        domains: normalizeDomainList(obj.domains),
        dnsType: String(obj.dnsType || "").trim(),
        credentials: normalizeStringRecord(obj.credentials),
        updatedAt:
          normalizeTimestamp(obj.updatedAt) || new Date().toISOString(),
      };
    } catch {
      return null;
    }
  }

  async saveAcmeSettings(
    value: Omit<AcmeSettings, "updatedAt">,
  ): Promise<AcmeSettings> {
    await this.ensureAcmeDataMigrated();
    const applications = await this.readAcmeApplicationsStore();
    const normalizedDomains = normalizeDomainList(value.domains);
    const primaryDomain = normalizedDomains[0] || "";
    const targetApplication =
      applications.find((item) => item.primaryDomain === primaryDomain) ||
      (applications.length === 1 ? applications[0]! : null);

    if (!targetApplication && applications.length > 1) {
      throw new Error("当前已存在多个申请项，请使用新接口管理 ACME 申请项");
    }

    const savedApplication = await this.saveAcmeApplication({
      id: targetApplication?.id,
      domains: normalizedDomains,
      dnsType: value.dnsType,
      credentials: value.credentials,
      renewEnabled: targetApplication?.renewEnabled ?? true,
      name: targetApplication?.name,
    });

    const next: AcmeSettings = {
      domains: savedApplication.domains,
      dnsType: savedApplication.dnsType,
      credentials: savedApplication.credentials,
      updatedAt: savedApplication.updatedAt,
    };
    await this.redis.set(this.acmeSettingsKey, JSON.stringify(next));
    return next;
  }

  async getAcmeSettings(): Promise<AcmeSettings | null> {
    await this.ensureAcmeDataMigrated();
    const applications = await this.readAcmeApplicationsStore();
    const application = applications[0];
    if (application) {
      return {
        domains: application.domains,
        dnsType: application.dnsType,
        credentials: application.credentials,
        updatedAt: application.updatedAt,
      };
    }
    return this.getAcmeSettingsLegacy();
  }

  async saveAcmeClientSettings(
    value: Pick<AcmeClientSettings, "certificateAuthority">,
  ): Promise<AcmeClientSettings> {
    const next: AcmeClientSettings = {
      certificateAuthority: normalizeAcmeCertificateAuthority(
        value.certificateAuthority,
      ),
      updatedAt: new Date().toISOString(),
    };
    await this.redis.set(this.acmeClientSettingsKey, JSON.stringify(next));
    return next;
  }

  async getAcmeClientSettings(): Promise<AcmeClientSettings | null> {
    const raw = await this.redis.get(this.acmeClientSettingsKey);
    if (!raw) return null;
    try {
      const obj = JSON.parse(raw);
      if (!obj || typeof obj !== "object") return null;
      return {
        certificateAuthority: normalizeAcmeCertificateAuthority(
          typeof obj.certificateAuthority === "string"
            ? obj.certificateAuthority
            : undefined,
        ),
        updatedAt:
          typeof obj.updatedAt === "string"
            ? obj.updatedAt
            : new Date().toISOString(),
      };
    } catch {
      return null;
    }
  }

  async ensureAcmeClientSettings(
    fallbackCertificateAuthority: AcmeCertificateAuthority = DEFAULT_ACME_CERTIFICATE_AUTHORITY,
  ): Promise<AcmeClientSettings> {
    const existing = await this.getAcmeClientSettings();
    if (existing) return existing;
    return this.saveAcmeClientSettings({
      certificateAuthority: fallbackCertificateAuthority,
    });
  }

  async saveAcmeCert(
    domain: string,
    cert: string,
    keyPem: string,
  ): Promise<void> {
    const k = `${this.acmeCertKey}${domain}`;
    await this.redis.set(k, JSON.stringify({ cert, key: keyPem }));
  }

  async getAcmeCert(
    domain: string,
  ): Promise<{ cert: string; key: string } | null> {
    const raw = await this.redis.get(`${this.acmeCertKey}${domain}`);
    if (!raw) return null;
    try {
      const obj = JSON.parse(raw);
      if (
        typeof obj?.cert === "string" &&
        typeof obj?.key === "string" &&
        obj.cert.trim() &&
        obj.key.trim()
      ) {
        return obj;
      }
      return null;
    } catch {
      return null;
    }
  }

  async deleteAcmeCert(domain: string): Promise<void> {
    await this.redis.del(`${this.acmeCertKey}${domain}`);
  }

  async getAcmeCertInfo(domain: string): Promise<SSLCertInfo | null> {
    const pair = await this.getAcmeCert(domain);
    if (!pair) return null;
    return this.parseCertInfo(pair.cert);
  }

  async saveAcmeCertFromFS(
    domain: string,
    opts?: { forceInstall?: boolean },
  ): Promise<boolean> {
    const { join } = await import("node:path");
    const { promises: fs } = await import("node:fs");

    const domainDir = join(dataPath, "ssl", domain);
    const installedKeyPath = join(domainDir, `${domain}.key`);
    const installedFullchainPath = join(domainDir, "fullchain.cer");

    try {
      const hasKey = await fileExists(installedKeyPath);
      const hasFullchain = await fileExists(installedFullchainPath);
      const shouldInstall = !!opts?.forceInstall || !hasKey || !hasFullchain;

      if (shouldInstall) {
        await fs.mkdir(domainDir, { recursive: true });
        const exists = await fileExists(ACME_EXECUTABLE_PATH);
        if (!exists) return false;

        const installProc = spawn(
          ACME_EXECUTABLE_PATH,
          [
            "--home",
            ACME_HOME_DIR,
            "--config-home",
            ACME_HOME_DIR,
            "--install-cert",
            "-d",
            domain,
            "--key-file",
            installedKeyPath,
            "--fullchain-file",
            installedFullchainPath,
          ],
          { stdio: ["ignore", "pipe", "pipe"] },
        );
        const installExitPromise = waitForProcessExit(installProc);

        const [, , exitCode] = await Promise.all([
          collectStreamOutput(installProc.stdout).catch(() => ""),
          collectStreamOutput(installProc.stderr).catch(() => ""),
          installExitPromise,
        ]);
        if (exitCode !== 0) return false;
      }

      const cert = await fs.readFile(installedFullchainPath, "utf-8");
      const key = await fs.readFile(installedKeyPath, "utf-8");
      if (!cert.trim() || !key.trim()) return false;
      if (!this.parseCertInfo(cert)) return false;
      await this.saveAcmeCert(domain, cert, key);
      return true;
    } catch {
      try {
        const fallbackHomes = [ACME_HOME_DIR, join(homedir(), ".acme.sh")];
        for (const home of fallbackHomes) {
          const certDir = join(home, domain);
          const certPathA = join(certDir, "fullchain.cer");
          const certPathB = join(certDir, `${domain}.cer`);
          const keyPath = join(certDir, `${domain}.key`);
          try {
            const cert = await fs
              .readFile(certPathA, "utf-8")
              .catch(async () => await fs.readFile(certPathB, "utf-8"));
            const key = await fs.readFile(keyPath, "utf-8");
            if (!cert.trim() || !key.trim()) continue;
            if (!this.parseCertInfo(cert)) continue;
            await this.saveAcmeCert(domain, cert, key);
            return true;
          } catch {
            // try next fallback directory
          }
        }
        return false;
      } catch {
        return false;
      }
    }
  }

  async updateRunType(
    run_type: RunType,
    reverse_proxy_submode?: ReverseProxySubmode,
  ): Promise<void> {
    const config = await this.getConfig();
    config.run_type = run_type;
    if (run_type === 1 && reverse_proxy_submode !== undefined) {
      config.reverse_proxy_submode = normalizeReverseProxySubmode(
        reverse_proxy_submode,
      );
    }

    if (run_type === 3) {
      config.proxy_mappings = [];
      config.default_route = DEFAULT_ROUTE_PLACEHOLDER;
    }

    await this.saveConfig(config);
  }

  async updateReverseProxySubmode(
    reverse_proxy_submode: ReverseProxySubmode,
  ): Promise<void> {
    const config = await this.getConfig();
    config.reverse_proxy_submode = normalizeReverseProxySubmode(
      reverse_proxy_submode,
    );
    await this.saveConfig(config);
  }

  async getRunModePromptPreferences(): Promise<RunModePromptPreferences> {
    const raw = await this.redis.get(this.runModePromptPreferencesKey);
    if (!raw) return DEFAULT_RUN_MODE_PROMPT_PREFERENCES;

    try {
      const parsed = JSON.parse(raw) as Partial<RunModePromptPreferences>;
      return {
        directToReverseProxy: parsed.directToReverseProxy === true,
        reverseProxyToDirect: parsed.reverseProxyToDirect === true,
        switchToSubdomain: parsed.switchToSubdomain === true,
        subdomainToReverseProxy: parsed.subdomainToReverseProxy === true,
      };
    } catch {
      return DEFAULT_RUN_MODE_PROMPT_PREFERENCES;
    }
  }

  async updateRunModePromptPreferences(
    patch: Partial<RunModePromptPreferences>,
  ): Promise<RunModePromptPreferences> {
    const next = {
      ...(await this.getRunModePromptPreferences()),
      ...patch,
    };
    await this.redis.set(
      this.runModePromptPreferencesKey,
      JSON.stringify(next),
    );
    return next;
  }

  async getProtocolMappingFeatureConfig(): Promise<ProtocolMappingFeatureConfig> {
    const raw = await this.redis.get(this.protocolMappingFeatureKey);
    if (!raw) return DEFAULT_PROTOCOL_MAPPING_FEATURE_CONFIG;

    try {
      const parsed = JSON.parse(raw) as Partial<ProtocolMappingFeatureConfig>;
      return normalizeProtocolMappingFeatureConfig(parsed);
    } catch {
      return DEFAULT_PROTOCOL_MAPPING_FEATURE_CONFIG;
    }
  }

  async updateProtocolMappingFeatureConfig(
    patch: Partial<ProtocolMappingFeatureConfig>,
  ): Promise<ProtocolMappingFeatureConfig> {
    const next = normalizeProtocolMappingFeatureConfig({
      ...(await this.getProtocolMappingFeatureConfig()),
      ...patch,
    });
    await this.redis.set(this.protocolMappingFeatureKey, JSON.stringify(next));
    return next;
  }

  async getFnosShareBypassConfig(): Promise<FnosShareBypassConfig> {
    const config = await this.getConfig();
    return normalizeFnosShareBypassConfig(config.fnos_share_bypass);
  }

  async getGatewayLoggingConfig(): Promise<GatewayLoggingSettings> {
    const config = await this.getConfig();
    return normalizeGatewayLoggingSettings(config.gateway_logging);
  }

  async getReverseProxyThrottleConfig(): Promise<ReverseProxyThrottleConfig> {
    const config = await this.getConfig();
    return normalizeReverseProxyThrottleConfig(config.reverse_proxy_throttle);
  }

  async updateFnosShareBypassConfig(
    patch: Partial<FnosShareBypassConfig>,
  ): Promise<FnosShareBypassConfig> {
    const config = await this.getConfig();
    const next = normalizeFnosShareBypassConfig({
      ...config.fnos_share_bypass,
      ...patch,
    });
    config.fnos_share_bypass = next;
    await this.saveConfig(config);
    return next;
  }

  async updateGatewayLoggingConfig(
    patch: Partial<GatewayLoggingSettings>,
  ): Promise<GatewayLoggingSettings> {
    const config = await this.getConfig();
    const next = normalizeGatewayLoggingSettings({
      ...config.gateway_logging,
      ...patch,
    });
    config.gateway_logging = next;
    await this.saveConfig(config);
    return next;
  }

  async updateReverseProxyThrottleConfig(
    patch: Partial<ReverseProxyThrottleConfig>,
  ): Promise<ReverseProxyThrottleConfig> {
    const config = await this.getConfig();
    const next = normalizeReverseProxyThrottleConfig({
      ...config.reverse_proxy_throttle,
      ...patch,
    });
    config.reverse_proxy_throttle = next;
    await this.saveConfig(config);
    return next;
  }

  async getTerminalFeatureConfig(): Promise<TerminalFeatureConfig> {
    const config = await this.getConfig();
    return normalizeTerminalFeatureConfig(config.terminal_feature);
  }

  async getAuthCredentialSettings(): Promise<AuthCredentialSettings> {
    const config = await this.getConfig();
    return normalizeAuthCredentialSettings(config.auth_credential_settings, {
      legacyAutoAddWhitelistOnLogin:
        config.subdomain_mode?.auto_add_whitelist_on_login,
    });
  }

  async updateAuthCredentialSettings(
    patch: Partial<AuthCredentialSettings>,
  ): Promise<AuthCredentialSettings> {
    const config = await this.getConfig();
    const next = normalizeAuthCredentialSettings(
      {
        ...config.auth_credential_settings,
        ...patch,
      },
      {
        legacyAutoAddWhitelistOnLogin:
          config.subdomain_mode?.auto_add_whitelist_on_login,
      },
    );
    config.auth_credential_settings = next;
    await this.saveConfig(config);
    return next;
  }

  async updateTerminalFeatureConfig(
    patch: Partial<TerminalFeatureConfig>,
  ): Promise<TerminalFeatureConfig> {
    const config = await this.getConfig();
    const next = normalizeTerminalFeatureConfig({
      ...config.terminal_feature,
      ...patch,
    });
    config.terminal_feature = next;
    await this.saveConfig(config);
    return next;
  }

  async getCaptchaSettings(): Promise<CaptchaSettings> {
    const raw = await this.redis.get(this.captchaSettingsKey);
    if (!raw) return DEFAULT_CAPTCHA_SETTINGS;

    try {
      const parsed = JSON.parse(raw) as Partial<CaptchaSettings>;
      return normalizeCaptchaSettings(parsed);
    } catch {
      return DEFAULT_CAPTCHA_SETTINGS;
    }
  }

  async updateCaptchaSettings(
    patch: Partial<CaptchaSettings>,
  ): Promise<CaptchaSettings> {
    const current = await this.getCaptchaSettings();
    const next = normalizeCaptchaSettings({
      ...current,
      ...patch,
      turnstile: {
        ...current.turnstile,
        ...(patch.turnstile ?? {}),
      },
    });
    await this.redis.set(this.captchaSettingsKey, JSON.stringify(next));
    return next;
  }

  async updateProxyMappings(mappings: ProxyMapping[]): Promise<void> {
    const config = await this.getConfig();
    config.proxy_mappings = mappings;
    await this.saveConfig(config);
  }

  async updateHostMappings(
    mappings: Array<Partial<HostMapping>>,
  ): Promise<void> {
    const config = await this.getConfig();
    config.host_mappings = normalizeHostMappings(mappings);
    await this.saveConfig(config);
  }

  async updateStreamMappings(
    mappings: Array<Partial<StreamMapping>>,
  ): Promise<void> {
    const config = await this.getConfig();
    config.stream_mappings = normalizeStreamMappings(mappings);
    await this.saveConfig(config);
  }

  async updateSubdomainModeConfig(
    patch: Partial<SubdomainModeConfig>,
  ): Promise<SubdomainModeConfig> {
    const config = await this.getConfig();
    const next = normalizeSubdomainModeConfig({
      ...config.subdomain_mode,
      ...patch,
    });
    config.subdomain_mode = next;
    await this.saveConfig(config);
    return next;
  }

  async updateSSLConfig(ssl: SSLConfig): Promise<void> {
    await this.saveSSLCertificate({
      label: "当前证书",
      source: "manual",
      cert: ssl.cert,
      key: ssl.key,
      activate: true,
      matchBy: {
        cert: ssl.cert,
        key: ssl.key,
      },
    });
  }

  async addIPBackoff(ip: string, ttlSeconds: number): Promise<void> {
    await this.redis.set(`fn_knock:backoff:${ip}`, "1", "EX", ttlSeconds);
  }

  async getIPBackoff(ip: string): Promise<boolean> {
    const val = await this.redis.get(`fn_knock:backoff:${ip}`);
    return val !== null;
  }

  async addNonce(nonce: string, ttlSeconds: number = 300): Promise<void> {
    await this.redis.set(`fn_knock:nonce:${nonce}`, "1", "EX", ttlSeconds);
  }

  /**
   * Stores a nonce if it doesn't exist. Returns true if it was set (new nonce), false if it already exists.
   */
  async setNonceIfNotExists(
    nonce: string,
    ttlSeconds: number = 600,
  ): Promise<boolean> {
    const key = `fn_knock:nonce:${nonce}`;
    const result = await this.redis.set(key, "1", "EX", ttlSeconds, "NX");
    return result === "OK";
  }

  /**
   * Stores a cron/distributed lock if it doesn't exist. Returns true when lock is acquired.
   */
  async setLockIfNotExists(
    lockName: string,
    ttlSeconds: number = 600,
  ): Promise<boolean> {
    const key = `fn_knock:lock:${lockName}`;
    const result = await this.redis.set(key, "1", "EX", ttlSeconds, "NX");
    return result === "OK";
  }

  async updateDefaultRoute(route: string): Promise<void> {
    const config = await this.getConfig();
    config.default_route = route;
    await this.saveConfig(config);
  }

  async updateDefaultTunnel(tunnel: "frp" | "cloudflared"): Promise<void> {
    const config = await this.getConfig();
    config.default_tunnel = tunnel;
    await this.saveConfig(config);
  }

  async getOnboardingStatus(): Promise<{ completed: boolean }> {
    const value = await this.redis.get(this.onboardingCompletedKey);
    return { completed: value === "1" };
  }

  async markOnboardingCompleted(): Promise<void> {
    await this.redis.set(this.onboardingCompletedKey, "1");
  }

  // CA Hosts list in Redis
  async getCAHosts(): Promise<string[]> {
    const raw = await this.redis.get(this.caHostsKey);
    if (!raw) return [];
    try {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed))
        return parsed.filter((x) => typeof x === "string");
    } catch {}
    return [];
  }

  async saveCAHosts(hosts: string[]): Promise<void> {
    await this.redis.set(this.caHostsKey, JSON.stringify(hosts));
  }

  async addCAHost(value: string): Promise<string[]> {
    const v = value.trim();
    if (!v) return await this.getCAHosts();
    const hosts = await this.getCAHosts();
    if (!hosts.includes(v)) {
      hosts.push(v);
      await this.saveCAHosts(hosts);
    }
    return hosts;
  }

  async removeCAHost(value: string): Promise<string[]> {
    const v = value.trim();
    const hosts = await this.getCAHosts();
    const next = hosts.filter((h) => h !== v);
    if (next.length !== hosts.length) {
      await this.saveCAHosts(next);
    }
    return next;
  }

  async clearCAHosts(): Promise<void> {
    await this.saveCAHosts([]);
  }

  // TOTP secret management
  private totpKey = "fn_knock:totp_secret";
  private totpListKey = "fn_knock:totps";
  private passkeyListKey = "fn_knock:passkeys";
  private passkeyChallengeKey = "fn_knock:passkey:challenge";
  private passkeyBindKey = "fn_knock:passkey:bind";

  async getTOTPCredentials(): Promise<TOTPCredential[]> {
    const raw = await this.redis.get(this.totpListKey);
    if (!raw) {
      // Migration for old single secret
      const oldSecret = await this.redis.get(this.totpKey);
      if (oldSecret) {
        const legacyTotp: TOTPCredential = {
          id: "legacy-totp-id",
          secret: oldSecret,
          comment: "默认凭据",
          createdAt: new Date().toISOString(),
        };
        await this.saveTOTPCredentials([legacyTotp]);
        await this.redis.del(this.totpKey);
        const passkeys = await this.getPasskeys();
        let passkeysModified = false;
        for (const pk of passkeys) {
          if (!pk.totpId) {
            pk.totpId = legacyTotp.id;
            passkeysModified = true;
          }
        }
        if (passkeysModified) await this.savePasskeys(passkeys);
        return [legacyTotp];
      }
      return [];
    }
    try {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) return parsed as TOTPCredential[];
    } catch {
      return [];
    }
    return [];
  }

  async saveTOTPCredentials(totps: TOTPCredential[]): Promise<void> {
    await this.redis.set(this.totpListKey, JSON.stringify(totps));
  }

  async addTOTPCredential(totp: TOTPCredential): Promise<void> {
    const totps = await this.getTOTPCredentials();
    totps.push(totp);
    await this.saveTOTPCredentials(totps);
  }

  async updateTOTPCredential(id: string, comment: string): Promise<boolean> {
    const totps = await this.getTOTPCredentials();
    const target = totps.find((t) => t.id === id);
    if (!target) return false;
    target.comment = comment;
    await this.saveTOTPCredentials(totps);
    return true;
  }

  async deleteTOTPCredential(id: string): Promise<boolean> {
    const totps = await this.getTOTPCredentials();
    const updated = totps.filter((t) => t.id !== id);
    if (updated.length === totps.length) return false;
    await this.saveTOTPCredentials(updated);

    // Cascade delete passkeys
    const passkeys = await this.getPasskeys();
    const remainingPasskeys = passkeys.filter((pk) => pk.totpId !== id);
    if (remainingPasskeys.length !== passkeys.length) {
      await this.savePasskeys(remainingPasskeys);
    }
    return true;
  }

  // Session management
  async addSession(
    sessionId: string,
    session: LoginSession,
    ttlSeconds: number,
  ): Promise<void> {
    await this.redis.set(
      `fn_knock:session:${sessionId}`,
      JSON.stringify(session),
      "EX",
      ttlSeconds,
    );
  }

  async getSession(sessionId: string): Promise<LoginSession | null> {
    const raw = await this.redis.get(`fn_knock:session:${sessionId}`);
    if (!raw) return null;
    try {
      const data = JSON.parse(raw) as LoginSession;
      return data;
    } catch {
      return null;
    }
  }

  async deleteSession(sessionId: string): Promise<void> {
    await this.redis.del(`fn_knock:session:${sessionId}`);
  }

  async updateSession(
    sessionId: string,
    updates: Partial<LoginSession>,
  ): Promise<LoginSession | null> {
    const key = `fn_knock:session:${sessionId}`;
    const [raw, ttl] = await Promise.all([
      this.redis.get(key),
      this.redis.ttl(key),
    ]);
    if (!raw) return null;

    try {
      const current = JSON.parse(raw) as LoginSession;
      const next: LoginSession = {
        ...current,
        ...updates,
      };

      if (ttl > 0) {
        await this.redis.set(key, JSON.stringify(next), "EX", ttl);
      } else {
        await this.redis.set(key, JSON.stringify(next));
      }
      return next;
    } catch {
      return null;
    }
  }

  async isValidSession(sessionId: string): Promise<boolean> {
    const val = await this.redis.get(`fn_knock:session:${sessionId}`);
    return val !== null;
  }

  async listSessions(): Promise<Array<{ id: string; data: LoginSession }>> {
    const match = "fn_knock:session:*";
    let cursor = "0";
    const keys: string[] = [];
    do {
      const res = await this.redis.scan(cursor, "MATCH", match, "COUNT", 100);
      cursor = res[0];
      const batch = res[1] as string[];
      if (batch && batch.length) keys.push(...batch);
    } while (cursor !== "0");
    if (keys.length === 0) return [];
    const values = await this.redis.mget(keys);
    const list: Array<{ id: string; data: LoginSession }> = [];
    keys.forEach((key, idx) => {
      const raw = values[idx];
      if (!raw) return;
      try {
        const data = JSON.parse(raw) as LoginSession;
        const id = key.replace("fn_knock:session:", "");
        list.push({ id, data });
      } catch {}
    });
    return list.sort((a, b) => {
      const at = Date.parse(a.data.loginTime) || 0;
      const bt = Date.parse(b.data.loginTime) || 0;
      return bt - at;
    });
  }

  async getPasskeys(): Promise<PasskeyCredential[]> {
    const raw = await this.redis.get(this.passkeyListKey);
    if (!raw) return [];
    try {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) return parsed as PasskeyCredential[];
    } catch {
      return [];
    }
    return [];
  }

  async savePasskeys(passkeys: PasskeyCredential[]): Promise<void> {
    await this.redis.set(this.passkeyListKey, JSON.stringify(passkeys));
  }

  async addPasskey(passkey: PasskeyCredential): Promise<void> {
    const passkeys = await this.getPasskeys();
    passkeys.push(passkey);
    await this.savePasskeys(passkeys);
  }

  async deletePasskey(id: string): Promise<boolean> {
    const passkeys = await this.getPasskeys();
    const updated = passkeys.filter((passkey) => passkey.id !== id);
    if (updated.length === passkeys.length) return false;
    await this.savePasskeys(updated);
    return true;
  }

  async updatePasskeyCounter(
    id: string,
    counter: number,
    lastUsedAt: string,
  ): Promise<boolean> {
    const passkeys = await this.getPasskeys();
    const target = passkeys.find((passkey) => passkey.id === id);
    if (!target) return false;
    target.counter = counter;
    target.lastUsedAt = lastUsedAt;
    await this.savePasskeys(passkeys);
    return true;
  }

  async setPasskeyChallenge(
    challenge: string,
    type: "register" | "auth",
    ttlSeconds: number = 300,
  ): Promise<void> {
    await this.redis.set(
      `${this.passkeyChallengeKey}:${challenge}`,
      type,
      "EX",
      ttlSeconds,
    );
  }

  async consumePasskeyChallenge(
    challenge: string,
    type: "register" | "auth",
  ): Promise<boolean> {
    const key = `${this.passkeyChallengeKey}:${challenge}`;
    const value = await this.redis.get(key);
    if (value !== type) return false;
    await this.redis.del(key);
    return true;
  }

  async createPasskeyBindToken(
    totpId: string,
    ttlSeconds: number = 600,
  ): Promise<string> {
    const token = randomBytes(24).toString("hex");
    await this.redis.set(
      `${this.passkeyBindKey}:${token}`,
      totpId,
      "EX",
      ttlSeconds,
    );
    return token;
  }

  async isPasskeyBindTokenValid(token: string): Promise<boolean> {
    const value = await this.redis.get(`${this.passkeyBindKey}:${token}`);
    return value !== null;
  }

  async consumePasskeyBindToken(token: string): Promise<string | null> {
    const key = `${this.passkeyBindKey}:${token}`;
    const value = await this.redis.get(key);
    if (!value) return null;
    await this.redis.del(key);
    return value;
  }
}

export const configManager = new ConfigManager();
