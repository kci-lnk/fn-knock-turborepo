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

const REDIS_CONFIG = {
  host: process.env.REDIS_HOST || "127.0.0.1",
  port: parseInt(process.env.REDIS_PORT || "6379"),
  password: process.env.REDIS_PASSWORD,
};

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
export type AcmeJob = {
  id: string;
  domains: string[];
  method: AcmeJobMethod;
  provider: string | null;
  createdAt: string;
  status: AcmeJobStatus;
  progress: number;
  message?: string;
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
  comment?: string;
  ip: string;
  userAgent: string;
  loginTime: string;
  expiresAt?: string;
  ipLocation?: string;
};

export interface AppConfig {
  run_type: RunType;
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
  auth_credential_settings?: AuthCredentialSettings;
  terminal_feature?: TerminalFeatureConfig;
}

export interface RunModePromptPreferences {
  directToReverseProxy: boolean;
  reverseProxyToDirect: boolean;
}

export interface AuthCredentialSettings {
  session_ttl_seconds: number;
  remember_me_ttl_seconds: number;
}

export const DEFAULT_AUTH_CREDENTIAL_SETTINGS: AuthCredentialSettings = {
  session_ttl_seconds: 24 * 3600,
  remember_me_ttl_seconds: 365 * 24 * 3600,
};

const DEFAULT_GATEWAY_LOGGING_SETTINGS: GatewayLoggingSettings = {
  enabled: false,
  max_days: 7,
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

const DEFAULT_CONFIG: AppConfig = {
  run_type: 1,
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
  default_route: "/__select__",
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

const normalizeAuthCredentialSettings = (
  value?: Partial<AuthCredentialSettings> | null,
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

  return {
    session_ttl_seconds: sessionTtlSeconds,
    remember_me_ttl_seconds: rememberMeTtlSeconds,
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
    cert,
    key,
    created_at: createdAt,
    updated_at: updatedAt,
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
    preserve_host: raw.preserve_host !== false,
    service_role: serviceRole,
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
  private caHostsKey = "fn_knock:ca:hosts";
  private acmeJobKey = "fn_knock:acme:job:";
  private acmeLogsKey = "fn_knock:acme:logs:";
  private acmeCertKey = "fn_knock:acme:cert:";
  private acmeSettingsKey = "fn_knock:acme:settings";
  private acmeClientSettingsKey = "fn_knock:acme:client-settings";
  private onboardingCompletedKey = "fn_knock:onboarding:completed";
  private runModePromptPreferencesKey = "fn_knock:run-mode:prompt-preferences";

  constructor() {
    this.redis = redis;
  }

  async getConfig(): Promise<AppConfig> {
    try {
      const data = await this.redis.get(this.configKey);
      if (data) {
        // 处理已有数据缺少 default_route 的兼容情况
        const parsed = JSON.parse(data) as AppConfig;
        if (![0, 1, 3].includes(parsed.run_type)) parsed.run_type = 1;
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
        parsed.auth_credential_settings = normalizeAuthCredentialSettings(
          parsed.auth_credential_settings,
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
      auth_credential_settings: { ...DEFAULT_AUTH_CREDENTIAL_SETTINGS },
      terminal_feature: { ...DEFAULT_TERMINAL_FEATURE_CONFIG },
    };
  }

  /**
   * 返回不含 SSL cert/key 原文的配置（供 /api/admin/config 使用）
   */
  async getConfigSafe(): Promise<any> {
    const config = await this.getConfig();
    const { ssl, ...rest } = config;
    return {
      ...rest,
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
    cert: string;
    key: string;
    activate?: boolean;
    matchBy?: {
      source?: SSLCertificateSource;
      primary_domain?: string;
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

  async createAcmeJob(job: AcmeJob): Promise<void> {
    const key = `${this.acmeJobKey}${job.id}`;
    await this.redis.set(key, JSON.stringify(job), "EX", 86400);
  }

  async updateAcmeJob(id: string, patch: Partial<AcmeJob>): Promise<void> {
    const key = `${this.acmeJobKey}${id}`;
    const raw = await this.redis.get(key);
    if (!raw) return;
    let obj: AcmeJob;
    try {
      obj = JSON.parse(raw) as AcmeJob;
    } catch {
      return;
    }
    const next = { ...obj, ...patch };
    await this.redis.set(key, JSON.stringify(next), "EX", 86400);
  }

  async getAcmeJob(id: string): Promise<AcmeJob | null> {
    const raw = await this.redis.get(`${this.acmeJobKey}${id}`);
    if (!raw) return null;
    try {
      return JSON.parse(raw) as AcmeJob;
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

  async saveAcmeSettings(
    value: Omit<AcmeSettings, "updatedAt">,
  ): Promise<AcmeSettings> {
    const next: AcmeSettings = {
      ...value,
      updatedAt: new Date().toISOString(),
    };
    await this.redis.set(this.acmeSettingsKey, JSON.stringify(next));
    return next;
  }

  async getAcmeSettings(): Promise<AcmeSettings | null> {
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
      return obj as AcmeSettings;
    } catch {
      return null;
    }
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

  async updateRunType(run_type: RunType): Promise<void> {
    const config = await this.getConfig();
    config.run_type = run_type;
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

  async getFnosShareBypassConfig(): Promise<FnosShareBypassConfig> {
    const config = await this.getConfig();
    return normalizeFnosShareBypassConfig(config.fnos_share_bypass);
  }

  async getGatewayLoggingConfig(): Promise<GatewayLoggingSettings> {
    const config = await this.getConfig();
    return normalizeGatewayLoggingSettings(config.gateway_logging);
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

  async getTerminalFeatureConfig(): Promise<TerminalFeatureConfig> {
    const config = await this.getConfig();
    return normalizeTerminalFeatureConfig(config.terminal_feature);
  }

  async getAuthCredentialSettings(): Promise<AuthCredentialSettings> {
    const config = await this.getConfig();
    return normalizeAuthCredentialSettings(config.auth_credential_settings);
  }

  async updateAuthCredentialSettings(
    patch: Partial<AuthCredentialSettings>,
  ): Promise<AuthCredentialSettings> {
    const config = await this.getConfig();
    const next = normalizeAuthCredentialSettings({
      ...config.auth_credential_settings,
      ...patch,
    });
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
