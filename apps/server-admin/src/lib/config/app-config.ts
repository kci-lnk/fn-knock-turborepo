import type { AutoHttpsConfig } from "../auto-https-redirect";
import { DEFAULT_AUTO_MANAGE_FIREWALL } from "../firewall-automation";
import { normalizeGatewayPortalConfigValue } from "../gateway-portal-config";
import { normalizeIp } from "../ip-normalize";
import { DEFAULT_REVERSE_PROXY_SUBMODE } from "../reverse-proxy-submode";
import { DEFAULT_SSH_SECURITY_CONFIG } from "../ssh-security/config";
import {
  DEFAULT_TERMINAL_FEATURE_CONFIG,
  type TerminalFeatureConfig,
} from "../terminal-shared";
import { normalizeCidrLines } from "../../../../../packages/admin-shared/src/utils/cidr";
import { normalizeAppearanceConfig } from "../../../../../packages/admin-shared/src/utils/appearance";
import {
  DEFAULT_AUTH_CREDENTIAL_SETTINGS,
  DEFAULT_APPEARANCE_CONFIG_FOR_ADMIN,
  DEFAULT_AUTO_HTTPS_CONFIG,
  DEFAULT_DASHBOARD_DISPLAY_CONFIG,
  DEFAULT_EVENT_SYSTEM_CONFIG,
  DEFAULT_FNOS_PORT_ICON_HIJACK_CONFIG,
  DEFAULT_FNOS_SHARE_BYPASS_CONFIG,
  DEFAULT_GATEWAY_CRAWLER_BLOCKER_CONFIG,
  DEFAULT_GATEWAY_HOST_RESPONSE_CONFIG,
  DEFAULT_GATEWAY_HOST_RESPONSE_RUNTIME_STATE,
  DEFAULT_GATEWAY_LOGGING_SETTINGS,
  DEFAULT_GATEWAY_PORTAL_CONFIG,
  DEFAULT_GATEWAY_PROXY_HEADERS_CONFIG,
  DEFAULT_GATEWAY_PROXY_HEADERS_RUNTIME_STATE,
  DEFAULT_GATEWAY_VISIBILITY_CONFIG,
  DEFAULT_GATEWAY_VISIBILITY_RUNTIME_STATE,
  DEFAULT_IP_LOCATION_API_CONFIG,
  DEFAULT_LOCALE_CONFIG,
  DEFAULT_PROTOCOL_MAPPING_FEATURE_CONFIG,
  DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG,
  DEFAULT_REVERSE_PROXY_TRUSTED_IP_RUNTIME_STATE,
  DEFAULT_RUN_MODE_PROMPT_PREFERENCES,
  DEFAULT_SCAN_DISCOVERY_CONFIG,
  DEFAULT_SMART_CONNECT_CONFIG,
  DEFAULT_SMART_CONNECT_RUNTIME_STATE,
  DEFAULT_WAF_CONFIG,
} from "./defaults";
import {
  normalizeBoundedInt,
  normalizeOptionalString,
  normalizePositiveInt,
  normalizeScanDiscoveryConfig,
  normalizeStringList,
} from "./normalizers";
import {
  DEFAULT_SUBDOMAIN_AUTH_CACHE_TTL_SECONDS,
  DEFAULT_SUBDOMAIN_AUTH_CACHE_UNAUTHORIZED_TTL_SECONDS,
  DEFAULT_SUBDOMAIN_AUTH_TARGET,
  normalizeHost,
} from "./mapping-normalizers";
import { normalizeDomainList } from "./certificate-normalizers";
import type {
  AcmeApplication,
  AcmeApplicationLatestJobStatus,
  AcmeIssuedCertificate,
  AcmeJob,
  AcmeJobTrigger,
  AcmeRuntimeLock,
  AppConfig,
  AuthCredentialSettings,
  CaptchaSettings,
  DashboardDisplayConfig,
  EventSystemConfig,
  EventSystemResourceAlertRuleConfig,
  EventSystemSimpleRuleConfig,
  FnosPortIconHijackConfig,
  FnosShareBypassConfig,
  GatewayCrawlerBlockerConfig,
  GatewayHostResponseConfig,
  GatewayHostResponseRuntimeState,
  GatewayLoggingSettings,
  GatewayPortalConfig,
  GatewayProxyHeadersConfig,
  GatewayProxyHeadersRuntimeState,
  GatewayVisibilityConfig,
  GatewayVisibilityRuntimeState,
  GatewayVisibilitySelection,
  IpLocationApiConfig,
  PostLoginIpGrantMode,
  ProtocolMappingFeatureConfig,
  ReverseProxyThrottleConfig,
  ReverseProxyTrustedIPRuntimeState,
  RunType,
  SmartConnectConfig,
  SmartConnectRuntimeState,
  SSLCertInfo,
  SSLConfig,
  SSLDeploymentMode,
  SSLManagedCertificate,
  SSLCertificateSource,
  TurnstileCaptchaConfig,
} from "./types";

export const DEFAULT_ROUTE_PLACEHOLDER = "/__select__";
export const DEFAULT_RUN_TYPE: RunType = 3;

export const DEFAULT_CONFIG: AppConfig = {
  run_type: DEFAULT_RUN_TYPE,
  reverse_proxy_submode: DEFAULT_REVERSE_PROXY_SUBMODE,
  auto_manage_firewall: DEFAULT_AUTO_MANAGE_FIREWALL,
  whitelist_ips: [],
  proxy_mappings: [],
  host_mappings: [],
  stream_mappings: [],
  subdomain_mode: {
    root_domain: "",
    auth_host: "",
    auth_target: DEFAULT_SUBDOMAIN_AUTH_TARGET,
    cookie_domain: "",
    edge_client_ip_enabled: false,
    aliyun_esa_enabled: false,
    tencent_edgeone_enabled: false,
    public_auth_base_url: "",
    public_http_port: 0,
    public_https_port: 0,
    auth_cache_ttl_seconds: DEFAULT_SUBDOMAIN_AUTH_CACHE_TTL_SECONDS,
    auth_cache_unauthorized_ttl_seconds:
      DEFAULT_SUBDOMAIN_AUTH_CACHE_UNAUTHORIZED_TTL_SECONDS,
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
  fnos_port_icon_hijack: {
    enabled: false,
    updated_at: null,
  },
  gateway_logging: {
    ...DEFAULT_GATEWAY_LOGGING_SETTINGS,
  },
  waf: {
    ...DEFAULT_WAF_CONFIG,
    disabled_hosts: [],
    disabled_path_prefixes: [],
  },
  reverse_proxy_throttle: {
    ...DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG,
  },
  gateway_visibility: {
    ...DEFAULT_GATEWAY_VISIBILITY_CONFIG,
    selections: [],
    custom_cidrs: [],
  },
  gateway_proxy_headers: {
    ...DEFAULT_GATEWAY_PROXY_HEADERS_CONFIG,
    disabled_hosts: [],
  },
  gateway_host_response: {
    ...DEFAULT_GATEWAY_HOST_RESPONSE_CONFIG,
    disabled_hosts: [],
  },
  gateway_crawler_blocker: {
    ...DEFAULT_GATEWAY_CRAWLER_BLOCKER_CONFIG,
  },
  gateway_portal: {
    ...DEFAULT_GATEWAY_PORTAL_CONFIG,
  },
  appearance: {
    ...DEFAULT_APPEARANCE_CONFIG_FOR_ADMIN,
  },
  dashboard_display: {
    ...DEFAULT_DASHBOARD_DISPLAY_CONFIG,
  },
  auto_https: {
    ...DEFAULT_AUTO_HTTPS_CONFIG,
  },
  smart_connect: {
    ...DEFAULT_SMART_CONNECT_CONFIG,
  },
  scan_discovery: {
    ...DEFAULT_SCAN_DISCOVERY_CONFIG,
    custom_cidrs: [],
    selected_cidrs: [],
  },
  auth_credential_settings: {
    ...DEFAULT_AUTH_CREDENTIAL_SETTINGS,
  },
  event_system: {
    ...DEFAULT_EVENT_SYSTEM_CONFIG,
    rules: {
      login_failure: {
        ...DEFAULT_EVENT_SYSTEM_CONFIG.rules.login_failure,
      },
      ip_drift: {
        ...DEFAULT_EVENT_SYSTEM_CONFIG.rules.ip_drift,
      },
      scanner_blocked: {
        ...DEFAULT_EVENT_SYSTEM_CONFIG.rules.scanner_blocked,
      },
      ddns_update: {
        ...DEFAULT_EVENT_SYSTEM_CONFIG.rules.ddns_update,
      },
      gateway_throttle_block: {
        ...DEFAULT_EVENT_SYSTEM_CONFIG.rules.gateway_throttle_block,
      },
      waf_blocked: {
        ...DEFAULT_EVENT_SYSTEM_CONFIG.rules.waf_blocked,
      },
      app_update_available: {
        ...DEFAULT_EVENT_SYSTEM_CONFIG.rules.app_update_available,
      },
      frp_tunnel: {
        ...DEFAULT_EVENT_SYSTEM_CONFIG.rules.frp_tunnel,
      },
      cloudflared_tunnel: {
        ...DEFAULT_EVENT_SYSTEM_CONFIG.rules.cloudflared_tunnel,
      },
      ssh_login_success: {
        ...DEFAULT_EVENT_SYSTEM_CONFIG.rules.ssh_login_success,
      },
      ssh_login_failure: {
        ...DEFAULT_EVENT_SYSTEM_CONFIG.rules.ssh_login_failure,
      },
      ssh_ip_blocked: {
        ...DEFAULT_EVENT_SYSTEM_CONFIG.rules.ssh_ip_blocked,
      },
      cpu_alert: {
        ...DEFAULT_EVENT_SYSTEM_CONFIG.rules.cpu_alert,
      },
      memory_alert: {
        ...DEFAULT_EVENT_SYSTEM_CONFIG.rules.memory_alert,
      },
    },
  },
  terminal_feature: {
    ...DEFAULT_TERMINAL_FEATURE_CONFIG,
  },
  ssh_security: {
    ...DEFAULT_SSH_SECURITY_CONFIG,
    allowed_regions: [],
    custom_cidrs: [],
  },
  locale: {
    ...DEFAULT_LOCALE_CONFIG,
  },
};

export const normalizeGatewayLoggingSettings = (
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

export const normalizeReverseProxyThrottleConfig = (
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

export const normalizeEventSystemSimpleRuleConfig = (
  value?: Partial<EventSystemSimpleRuleConfig> | null,
  fallback: EventSystemSimpleRuleConfig = { enabled: true },
): EventSystemSimpleRuleConfig => {
  const raw = value ?? {};

  return {
    enabled: typeof raw.enabled === "boolean" ? raw.enabled : fallback.enabled,
  };
};

export const normalizeEventSystemResourceAlertRuleConfig = (
  value?: Partial<EventSystemResourceAlertRuleConfig> | null,
  fallback: EventSystemResourceAlertRuleConfig = DEFAULT_EVENT_SYSTEM_CONFIG
    .rules.cpu_alert,
): EventSystemResourceAlertRuleConfig => {
  const raw = value ?? {};
  const thresholdPercent = normalizeBoundedInt(
    raw.threshold_percent,
    fallback.threshold_percent,
    {
      min: 1,
      max: 100,
    },
  );
  const recoverPercent = normalizeBoundedInt(
    raw.recover_percent,
    fallback.recover_percent,
    {
      min: 0,
      max: thresholdPercent,
    },
  );

  return {
    enabled: typeof raw.enabled === "boolean" ? raw.enabled : fallback.enabled,
    threshold_percent: thresholdPercent,
    recover_percent: recoverPercent,
    sample_interval_seconds: normalizePositiveInt(
      raw.sample_interval_seconds,
      fallback.sample_interval_seconds,
      {
        min: 5,
        max: 3600,
      },
    ),
    sustain_seconds: normalizePositiveInt(
      raw.sustain_seconds,
      fallback.sustain_seconds,
      {
        min: 10,
        max: 24 * 3600,
      },
    ),
  };
};

export const normalizeEventSystemConfig = (
  value?: Partial<EventSystemConfig> | null,
): EventSystemConfig => {
  const raw = value ?? {};
  const rawRules =
    (raw.rules as
      | (Partial<EventSystemConfig["rules"]> & {
          login_failure_threshold?: Partial<EventSystemSimpleRuleConfig> & {
            count?: unknown;
          };
        })
      | undefined) ?? {};

  return {
    enabled:
      typeof raw.enabled === "boolean"
        ? raw.enabled
        : DEFAULT_EVENT_SYSTEM_CONFIG.enabled,
    retention_days: normalizePositiveInt(
      raw.retention_days,
      DEFAULT_EVENT_SYSTEM_CONFIG.retention_days,
      {
        min: 1,
        max: 90,
      },
    ),
    rules: {
      login_failure: normalizeEventSystemSimpleRuleConfig(
        rawRules.login_failure ?? rawRules.login_failure_threshold,
        DEFAULT_EVENT_SYSTEM_CONFIG.rules.login_failure,
      ),
      ip_drift: normalizeEventSystemSimpleRuleConfig(
        rawRules.ip_drift,
        DEFAULT_EVENT_SYSTEM_CONFIG.rules.ip_drift,
      ),
      scanner_blocked: normalizeEventSystemSimpleRuleConfig(
        rawRules.scanner_blocked,
        DEFAULT_EVENT_SYSTEM_CONFIG.rules.scanner_blocked,
      ),
      ddns_update: normalizeEventSystemSimpleRuleConfig(
        rawRules.ddns_update,
        DEFAULT_EVENT_SYSTEM_CONFIG.rules.ddns_update,
      ),
      gateway_throttle_block: normalizeEventSystemSimpleRuleConfig(
        rawRules.gateway_throttle_block,
        DEFAULT_EVENT_SYSTEM_CONFIG.rules.gateway_throttle_block,
      ),
      waf_blocked: normalizeEventSystemSimpleRuleConfig(
        rawRules.waf_blocked,
        DEFAULT_EVENT_SYSTEM_CONFIG.rules.waf_blocked,
      ),
      app_update_available: normalizeEventSystemSimpleRuleConfig(
        rawRules.app_update_available,
        DEFAULT_EVENT_SYSTEM_CONFIG.rules.app_update_available,
      ),
      frp_tunnel: normalizeEventSystemSimpleRuleConfig(
        rawRules.frp_tunnel,
        DEFAULT_EVENT_SYSTEM_CONFIG.rules.frp_tunnel,
      ),
      cloudflared_tunnel: normalizeEventSystemSimpleRuleConfig(
        rawRules.cloudflared_tunnel,
        DEFAULT_EVENT_SYSTEM_CONFIG.rules.cloudflared_tunnel,
      ),
      ssh_login_success: normalizeEventSystemSimpleRuleConfig(
        rawRules.ssh_login_success,
        DEFAULT_EVENT_SYSTEM_CONFIG.rules.ssh_login_success,
      ),
      ssh_login_failure: normalizeEventSystemSimpleRuleConfig(
        rawRules.ssh_login_failure,
        DEFAULT_EVENT_SYSTEM_CONFIG.rules.ssh_login_failure,
      ),
      ssh_ip_blocked: normalizeEventSystemSimpleRuleConfig(
        rawRules.ssh_ip_blocked,
        DEFAULT_EVENT_SYSTEM_CONFIG.rules.ssh_ip_blocked,
      ),
      cpu_alert: normalizeEventSystemResourceAlertRuleConfig(
        rawRules.cpu_alert,
        DEFAULT_EVENT_SYSTEM_CONFIG.rules.cpu_alert,
      ),
      memory_alert: normalizeEventSystemResourceAlertRuleConfig(
        rawRules.memory_alert,
        DEFAULT_EVENT_SYSTEM_CONFIG.rules.memory_alert,
      ),
    },
  };
};

export const normalizeGatewayVisibilitySelection = (
  value?: Partial<GatewayVisibilitySelection> | null,
): GatewayVisibilitySelection | null => {
  const raw = value ?? {};
  const province = normalizeOptionalString(raw.province);
  const label = normalizeOptionalString(raw.label);
  const valueLabel = normalizeOptionalString(raw.value);
  const city = normalizeOptionalString(raw.city);
  const queryCity = normalizeOptionalString(raw.query_city);

  if (!province || !label || !valueLabel) {
    return null;
  }

  return {
    province,
    city: city || null,
    label,
    value: valueLabel,
    query_city: queryCity || null,
    is_province_wide: raw.is_province_wide === true,
    is_municipality: raw.is_municipality === true,
  };
};

export const normalizeGatewayVisibilityConfig = (
  value?: Partial<GatewayVisibilityConfig> | null,
): GatewayVisibilityConfig => {
  const raw = value ?? {};

  return {
    enabled: raw.enabled === true,
    selections: Array.isArray(raw.selections)
      ? raw.selections
          .map((item) => normalizeGatewayVisibilitySelection(item))
          .filter((item): item is GatewayVisibilitySelection => item !== null)
      : [],
    custom_cidrs: normalizeCidrLines(
      Array.isArray(raw.custom_cidrs)
        ? raw.custom_cidrs.map((item) => String(item ?? ""))
        : [],
    ),
  };
};

export const normalizeGatewayVisibilityRuntimeState = (
  value?: Partial<GatewayVisibilityRuntimeState> | null,
): GatewayVisibilityRuntimeState => {
  const raw = value ?? {};
  const updatedAt = normalizeOptionalString(raw.updated_at);

  return {
    enabled: raw.enabled === true,
    cidrs: normalizeCidrLines(
      Array.isArray(raw.cidrs)
        ? raw.cidrs.map((item) => String(item ?? ""))
        : [],
    ),
    updated_at: updatedAt || null,
  };
};

export const normalizeGatewayProxyHeadersConfig = (
  value?: Partial<GatewayProxyHeadersConfig> | null,
): GatewayProxyHeadersConfig => {
  const raw = value ?? {};

  return {
    disabled_hosts: Array.isArray(raw.disabled_hosts)
      ? [
          ...new Set(raw.disabled_hosts.map((item) => normalizeHost(item))),
        ].filter(Boolean)
      : [],
  };
};

export const normalizeGatewayProxyHeadersRuntimeState = (
  value?: Partial<GatewayProxyHeadersRuntimeState> | null,
): GatewayProxyHeadersRuntimeState => {
  const raw = value ?? {};
  const updatedAt = normalizeOptionalString(raw.updated_at);

  return {
    enabled: raw.enabled === true,
    omit_targets: normalizeStringList(raw.omit_targets),
    updated_at: updatedAt || null,
  };
};

export const normalizeGatewayHostResponseConfig = (
  value?: Partial<GatewayHostResponseConfig> | null,
): GatewayHostResponseConfig => {
  const raw = value ?? {};

  return {
    disabled_hosts: Array.isArray(raw.disabled_hosts)
      ? [
          ...new Set(raw.disabled_hosts.map((item) => normalizeHost(item))),
        ].filter(Boolean)
      : [],
  };
};

export const normalizeGatewayPortalConfig = normalizeGatewayPortalConfigValue;

export const normalizeGatewayHostResponseRuntimeState = (
  value?: Partial<GatewayHostResponseRuntimeState> | null,
): GatewayHostResponseRuntimeState => {
  const raw = value ?? {};
  const updatedAt = normalizeOptionalString(raw.updated_at);

  return {
    enabled: raw.enabled === true,
    omit_targets: normalizeStringList(raw.omit_targets),
    updated_at: updatedAt || null,
  };
};

export const normalizeGatewayCrawlerBlockerConfig = (
  value?: Partial<GatewayCrawlerBlockerConfig> | null,
): GatewayCrawlerBlockerConfig => {
  const raw = value ?? {};
  const updatedAt = normalizeOptionalString(raw.updated_at);

  return {
    enabled: raw.enabled === true,
    updated_at: updatedAt || null,
  };
};

export const normalizeReverseProxyTrustedIPRuntimeState = (
  value?: Partial<ReverseProxyTrustedIPRuntimeState> | null,
): ReverseProxyTrustedIPRuntimeState => {
  const raw = value ?? {};
  const updatedAt = normalizeOptionalString(raw.updated_at);
  const sourceMap = new Map<string, Set<string>>();

  for (const item of Array.isArray(raw.items) ? raw.items : []) {
    const normalizedIp = normalizeIp(
      item && typeof item === "object" && "ip" in item ? item.ip : "",
    );
    if (!normalizedIp) continue;

    const sources = normalizeStringList(
      item && typeof item === "object" && "sources" in item ? item.sources : [],
    );
    const existing = sourceMap.get(normalizedIp) ?? new Set<string>();
    for (const source of sources) {
      existing.add(source);
    }
    sourceMap.set(normalizedIp, existing);
  }

  const items = [...sourceMap.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([ip, sources]) => ({
      ip,
      sources: [...sources].sort((left, right) => left.localeCompare(right)),
    }));

  return {
    enabled: raw.enabled === true,
    items,
    cidrs: normalizeCidrLines(
      Array.isArray(raw.cidrs)
        ? raw.cidrs.map((item) => String(item ?? ""))
        : [],
    ),
    updated_at: updatedAt || null,
  };
};

export const normalizeProtocolMappingFeatureConfig = (
  value?: Partial<ProtocolMappingFeatureConfig> | null,
): ProtocolMappingFeatureConfig => {
  const raw = value ?? {};

  return {
    enabled: raw.enabled === true,
  };
};

export const normalizeDashboardDisplayConfig = (
  value?: Partial<DashboardDisplayConfig> | null,
): DashboardDisplayConfig => {
  const raw = value ?? {};

  return {
    show_entry_status_module: raw.show_entry_status_module !== false,
  };
};

export { normalizeAppearanceConfig };

export const normalizeAutoHttpsConfig = (
  value?: Partial<AutoHttpsConfig> | null,
): AutoHttpsConfig => {
  const raw = value ?? {};

  return {
    enabled: raw.enabled === true,
  };
};

export const normalizeSmartConnectConfig = (
  value?: Partial<SmartConnectConfig> | null,
): SmartConnectConfig => {
  const raw = value ?? {};

  return {
    enabled: raw.enabled === true,
    selected_ipv4: normalizeOptionalString(raw.selected_ipv4) ?? "",
  };
};

export const normalizeSmartConnectRuntimeState = (
  value?: Partial<SmartConnectRuntimeState> | null,
): SmartConnectRuntimeState => {
  const raw = value ?? {};
  const lastSyncAt = normalizeOptionalString(raw.last_sync_at);
  const lastSyncError = normalizeOptionalString(raw.last_sync_error);

  return {
    selected_ipv4: normalizeOptionalString(raw.selected_ipv4) ?? "",
    synced_domains: normalizeDomainList(raw.synced_domains),
    managed_rule_count: normalizePositiveInt(raw.managed_rule_count, 0, {
      min: 0,
      max: 65535,
    }),
    last_sync_at: lastSyncAt || null,
    last_sync_error: lastSyncError || null,
  };
};

export const DEFAULT_CAPTCHA_SETTINGS: CaptchaSettings = {
  provider: "pow",
  widget_mode: "normal",
  pow: {},
  turnstile: {
    site_key: "",
    secret_key: "",
  },
};

export const normalizeIpLocationBaseUrl = (
  value: unknown,
  fallback = "",
): string => {
  if (typeof value !== "string") return fallback;
  const normalized = value.trim().replace(/\/+$/, "");
  return normalized || fallback;
};

export const normalizeIpLocationApiConfig = (
  value?: Partial<IpLocationApiConfig> | null,
): IpLocationApiConfig => {
  const raw = value ?? {};
  const ipLookupMode = raw.ip_lookup_mode === "custom" ? "custom" : "online";
  const cidrMode = raw.cidr_mode === "custom" ? "custom" : "online";

  return {
    ip_lookup_mode: ipLookupMode,
    ip_lookup_url:
      ipLookupMode === "custom"
        ? normalizeIpLocationBaseUrl(raw.ip_lookup_url)
        : DEFAULT_IP_LOCATION_API_CONFIG.ip_lookup_url,
    cidr_mode: cidrMode,
    cidr_url:
      cidrMode === "custom"
        ? normalizeIpLocationBaseUrl(raw.cidr_url)
        : DEFAULT_IP_LOCATION_API_CONFIG.cidr_url,
  };
};

export const normalizeFnosShareBypassConfig = (
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

export const normalizeFnosPortIconHijackConfig = (
  value?: Partial<FnosPortIconHijackConfig> | null,
): FnosPortIconHijackConfig => {
  const raw = value ?? {};

  return {
    enabled: raw.enabled === true,
    updated_at: normalizeOptionalString(raw.updated_at) ?? null,
  };
};

export const normalizePostLoginIpGrantMode = (
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

export const normalizeAuthCredentialSettings = (
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
  const sessionIpMobilityWindowSeconds = normalizePositiveInt(
    raw.session_ip_mobility_window_seconds,
    DEFAULT_AUTH_CREDENTIAL_SETTINGS.session_ip_mobility_window_seconds,
    { min: 60, max: 24 * 3600 },
  );

  return {
    session_ttl_seconds: sessionTtlSeconds,
    remember_me_ttl_seconds: rememberMeTtlSeconds,
    post_login_ip_grant_mode: postLoginIpGrantMode,
    post_login_ip_grant_ttl_seconds:
      postLoginIpGrantMode === "custom" ? postLoginIpGrantTtlSeconds : null,
    session_ip_mobility_enabled:
      typeof raw.session_ip_mobility_enabled === "boolean"
        ? raw.session_ip_mobility_enabled
        : DEFAULT_AUTH_CREDENTIAL_SETTINGS.session_ip_mobility_enabled,
    session_ip_mobility_window_seconds: sessionIpMobilityWindowSeconds,
    passkey_bind_prompt_enabled:
      typeof raw.passkey_bind_prompt_enabled === "boolean"
        ? raw.passkey_bind_prompt_enabled
        : DEFAULT_AUTH_CREDENTIAL_SETTINGS.passkey_bind_prompt_enabled,
  };
};

export const normalizeAuthCredentialSettingsPatch = (
  config: AppConfig,
  patch: Partial<AuthCredentialSettings>,
): AuthCredentialSettings =>
  normalizeAuthCredentialSettings(
    {
      ...config.auth_credential_settings,
      ...patch,
    },
    {
      legacyAutoAddWhitelistOnLogin:
        config.subdomain_mode?.auto_add_whitelist_on_login,
    },
  );

export {
  findMatchingSSLCertificate,
  hasSameNormalizedDomainSet,
  mirrorActiveSSLCertificate,
  normalizeAcmeApplication,
  normalizeAcmeIssuedCertificate,
  normalizeAcmeJob,
  normalizeAcmeRuntimeLock,
  normalizeDomainList,
  normalizeManagedSSLCertificate,
  normalizeSSLConfig,
  normalizeStringRecord,
  normalizeTimestamp,
} from "./certificate-normalizers";

export {
  DEFAULT_HOST_LOCATION_RESPONSE_CONTENT_TYPE,
  DEFAULT_SUBDOMAIN_AUTH_CACHE_TTL_SECONDS,
  DEFAULT_SUBDOMAIN_AUTH_CACHE_UNAUTHORIZED_TTL_SECONDS,
  DEFAULT_SUBDOMAIN_AUTH_TARGET,
  cleanHostLocationPath,
  createDisabledHostBasicAuth,
  forbiddenHostLocationResponseHeaders,
  isValidHTTPHeaderName,
  normalizeHost,
  normalizeHostAccessMode,
  normalizeHostBasicAuth,
  normalizeHostLocation,
  normalizeHostLocationAction,
  normalizeHostLocationMatch,
  normalizeHostLocationResponse,
  normalizeHostLocationResponseHeaders,
  normalizeHostLocations,
  normalizeHostMapping,
  normalizeHostMappings,
  normalizeHostServiceRole,
  normalizeStreamMapping,
  normalizeStreamMappings,
  normalizeStreamProtocol,
  normalizeSubdomainModeConfig,
} from "./mapping-normalizers";

export const normalizeCaptchaSettings = (
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
