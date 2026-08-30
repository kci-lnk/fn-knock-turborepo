import { extractPortFromTarget } from "@admin-shared/utils/extractPortFromTarget";
import {
  isHttpProxyTargetProtocol,
  isSupportedProxyTargetUrl,
} from "@admin-shared/utils/proxyTargetInput";
import { normalizeHostMappingAvailability } from "@/lib/host-mapping-availability";
import type {
  HostMapping,
  HostMappingTargetType,
  SubdomainModeConfig,
} from "@/types";
import {
  createDefaultStaticServe,
  normalizeHostMappingStaticServe,
  normalizeHostMappingTargetType,
  type StaticServeValidationIssue,
} from "./host-mapping-target-model";
import {
  DEFAULT_ACCESS_MODE,
  DEFAULT_PROTOCOL_MODE,
  DEFAULT_TARGET_PATH_MODE,
  type MappingInputMode,
} from "./subdomain-model-types";

export const parseTargetPort = (target: string): number | null => {
  const normalizedTarget = target.trim();
  if (!normalizedTarget) return null;

  const explicitPort = extractPortFromTarget(normalizedTarget);
  if (
    explicitPort !== null &&
    Number.isFinite(explicitPort) &&
    explicitPort > 0
  ) {
    return explicitPort;
  }

  try {
    const parsed = new URL(normalizedTarget);
    if (parsed.protocol === "https:" || parsed.protocol === "wss:") return 443;
    if (parsed.protocol === "http:" || parsed.protocol === "ws:") return 80;
  } catch {
    // Keep invalid targets unresolved; validation happens in the page flow.
  }

  return null;
};

export const isHttpTargetUrl = (target: string): boolean => {
  try {
    const parsed = new URL(target.trim());
    return (
      isHttpProxyTargetProtocol(parsed.protocol) && Boolean(parsed.hostname)
    );
  } catch {
    return false;
  }
};

export const resolveDefaultAuthServiceTarget = (
  draftTarget: string | null | undefined,
  savedTarget: string | null | undefined,
): string => {
  const defaultTarget = createDefaultModeForm().auth_target;
  const configuredTarget =
    draftTarget?.trim() || savedTarget?.trim() || defaultTarget;

  try {
    const parsed = new URL(configuredTarget);
    const port =
      parsed.port ||
      (parsed.protocol === "https:"
        ? "443"
        : parsed.protocol === "http:"
          ? "80"
          : "");

    if (!port) return configuredTarget;

    const normalized = new URL(`http://localhost:${port}`);
    normalized.pathname =
      parsed.pathname && parsed.pathname !== "/" ? parsed.pathname : "/";
    normalized.search = parsed.search;
    normalized.hash = parsed.hash;
    return normalized
      .toString()
      .replace(/\/$/, normalized.pathname === "/" ? "" : normalized.pathname);
  } catch {
    return configuredTarget || defaultTarget;
  }
};

export const isMappingDraftValid = ({
  basicAuthValidationMessage,
  canUseRootDomainSuffix,
  host,
  inputMode,
  staticServeValidationIssue,
  target,
  targetType,
}: {
  basicAuthValidationMessage: string;
  canUseRootDomainSuffix: boolean;
  host: string;
  inputMode: MappingInputMode;
  staticServeValidationIssue: StaticServeValidationIssue | null;
  target: string;
  targetType: HostMappingTargetType;
}): boolean => {
  if (!host) return false;
  if (inputMode === "subdomain" && !canUseRootDomainSuffix) {
    return false;
  }
  if (targetType !== "proxy") return staticServeValidationIssue === null;

  const normalizedTarget = target.trim();
  if (!normalizedTarget) return false;

  return (
    isSupportedProxyTargetUrl(normalizedTarget) && !basicAuthValidationMessage
  );
};

export const normalizePublicPort = (value: unknown): number => {
  const port =
    typeof value === "number"
      ? value
      : Number.parseInt(String(value ?? "").trim(), 10);
  if (!Number.isFinite(port) || port <= 0) return 0;
  return Math.floor(port);
};

export const parsePublicAuthBaseUrlPort = (
  value: string | undefined,
  scheme?: "http" | "https",
): number => {
  const trimmed = value?.trim();
  if (!trimmed) return 0;

  try {
    const parsed = new URL(trimmed);
    if (scheme && parsed.protocol !== `${scheme}:`) return 0;
    return normalizePublicPort(parsed.port);
  } catch {
    return 0;
  }
};

export const syncPublicAuthBaseUrlPort = (
  value: string | undefined,
  port: number,
): string => {
  const trimmed = value?.trim();
  if (!trimmed || !port) return trimmed || "";

  try {
    const parsed = new URL(trimmed);
    const scheme =
      parsed.protocol === "https:"
        ? "https"
        : parsed.protocol === "http:"
          ? "http"
          : null;
    if (!scheme) return "";

    const isDefaultPort =
      (scheme === "https" && port === 443) ||
      (scheme === "http" && port === 80);
    parsed.port = isDefaultPort ? "" : String(port);
    parsed.pathname = parsed.pathname.replace(/\/+$/, "") || "/";
    parsed.search = "";
    parsed.hash = "";
    return parsed.toString().replace(/\/$/, "");
  } catch {
    return "";
  }
};

export const resolveConfiguredAuthServicePublicPort = (
  config: Pick<
    SubdomainModeConfig,
    "public_auth_base_url" | "public_http_port" | "public_https_port"
  >,
  allowConfiguredPort = true,
): number => {
  const explicitHttpsPort = parsePublicAuthBaseUrlPort(
    config.public_auth_base_url,
    "https",
  );
  const explicitHttpPort = parsePublicAuthBaseUrlPort(
    config.public_auth_base_url,
    "http",
  );
  const configuredHttpsPort = normalizePublicPort(config.public_https_port);
  const configuredHttpPort = normalizePublicPort(config.public_http_port);
  return (
    explicitHttpsPort ||
    explicitHttpPort ||
    (allowConfiguredPort ? configuredHttpsPort || configuredHttpPort : 0)
  );
};

export const resolveConfiguredAccessEntryPublicPort = (
  config: Pick<
    SubdomainModeConfig,
    "public_auth_base_url" | "public_http_port" | "public_https_port"
  >,
  allowConfiguredPort = true,
): number => {
  const explicitHttpsPort = parsePublicAuthBaseUrlPort(
    config.public_auth_base_url,
    "https",
  );
  const explicitHttpPort = parsePublicAuthBaseUrlPort(
    config.public_auth_base_url,
    "http",
  );
  const configuredHttpsPort = normalizePublicPort(config.public_https_port);
  const configuredHttpPort = normalizePublicPort(config.public_http_port);
  const configuredPort =
    explicitHttpsPort ||
    explicitHttpPort ||
    (allowConfiguredPort ? configuredHttpsPort || configuredHttpPort : 0);
  return configuredPort > 0 ? configuredPort : 0;
};

export const isDefaultPublicPort = (value: unknown): boolean => {
  const port = normalizePublicPort(value);
  return port === 80 || port === 443;
};

export const formatHostWithOptionalPort = (
  host: string,
  port: string | number,
  shouldOmitPort: boolean,
): string => (shouldOmitPort ? host : `${host}:${port}`);

const configuredDefaultAuthPort = Number.parseInt(
  import.meta.env?.VITE_FN_KNOCK_DEFAULT_AUTH_PORT ?? "7997",
  10,
);
const defaultAuthPort =
  Number.isInteger(configuredDefaultAuthPort) &&
  configuredDefaultAuthPort >= 1 &&
  configuredDefaultAuthPort <= 65535
    ? configuredDefaultAuthPort
    : 7997;

export const createDefaultModeForm = (
  authPort = defaultAuthPort,
): SubdomainModeConfig => ({
  root_domain: "",
  auth_host: "",
  auth_target: `http://localhost:${authPort}`,
  cookie_domain: "",
  edge_client_ip_enabled: false,
  aliyun_esa_enabled: false,
  tencent_edgeone_enabled: false,
  public_auth_base_url: "",
  public_http_port: 0,
  public_https_port: 0,
  auth_cache_ttl_seconds: 1,
  auth_cache_unauthorized_ttl_seconds: 1,
  default_access_mode: "login_first",
  auto_add_whitelist_on_login: true,
  passkey_rp_mode: "auth_host",
  passkey_rp_id: "",
});

export const createDisabledMappingBasicAuth =
  (): HostMapping["basic_auth"] => ({
    enabled: false,
    username: "",
    password: "",
  });

export const normalizeMappingBasicAuth = (
  value?: Partial<HostMapping["basic_auth"]> | null,
): HostMapping["basic_auth"] => {
  const raw = value ?? {};
  const username = typeof raw.username === "string" ? raw.username.trim() : "";
  const password = typeof raw.password === "string" ? raw.password : "";

  if (raw.enabled !== true) {
    return createDisabledMappingBasicAuth();
  }

  return {
    enabled: true,
    username,
    password,
  };
};

export const normalizeBasicAuthProbeTarget = (value: string): string => {
  const trimmed = value.trim();
  if (!trimmed) return "";

  try {
    const parsed = new URL(trimmed);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return "";
    }
    parsed.hash = "";
    return parsed.toString();
  } catch {
    return "";
  }
};

export const createDefaultMapping = (): HostMapping => ({
  host: "",
  group_id: null,
  target_type: "proxy",
  target: "",
  static_serve: null,
  target_path_mode: DEFAULT_TARGET_PATH_MODE,
  waf_enabled: true,
  use_auth: true,
  access_mode: DEFAULT_ACCESS_MODE,
  suppress_toolbar: false,
  preserve_host: true,
  is_default: false,
  disabled: false,
  availability: null,
  visibility: createDefaultMappingVisibility(),
  protocol_mode: DEFAULT_PROTOCOL_MODE,
  basic_auth: createDisabledMappingBasicAuth(),
  locations: [],
  service_role: "app",
  title: "",
  title_override: "",
  favicon: "",
  favicon_override: "",
});

export const createDefaultMappingVisibility =
  (): HostMapping["visibility"] => ({
    mode: "inherit",
    selections: [],
    custom_cidrs: [],
    cidrs: [],
  });

export const normalizeMappingVisibility = (
  value: HostMapping["visibility"] | null | undefined,
): HostMapping["visibility"] => ({
  mode:
    value?.mode === "custom" || value?.mode === "disabled"
      ? value.mode
      : "inherit",
  selections: (value?.selections ?? []).map((selection) => ({ ...selection })),
  custom_cidrs: [...(value?.custom_cidrs ?? [])],
  cidrs: [...(value?.cidrs ?? [])],
});

export const buildBookmarkExportFilename = (rootDomain: string): string => {
  const normalizedRootDomain = rootDomain
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9.-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-+|-+$/g, "");

  return normalizedRootDomain
    ? `fn-knock-bookmarks-${normalizedRootDomain}.html`
    : "fn-knock-bookmarks.html";
};

export type HostMappingVisibilityIndicator = "inherit" | "custom" | null;

export interface HostMappingSecurityIndicatorState {
  customCidrCount: number;
  regionCount: number;
  visibility: HostMappingVisibilityIndicator;
  waf: boolean;
}

export const getMappingSecurityIndicatorState = ({
  globalVisibilityEnabled,
  globalWafEnabled,
  isAuthService,
  mapping,
}: {
  globalVisibilityEnabled: boolean;
  globalWafEnabled: boolean;
  isAuthService: boolean;
  mapping: HostMapping;
}): HostMappingSecurityIndicatorState => {
  const excluded = isAuthService || mapping.disabled === true;
  const visibilityMode = mapping.visibility?.mode;
  const visibility =
    excluded || !globalVisibilityEnabled || visibilityMode === "disabled"
      ? null
      : visibilityMode === "custom"
        ? "custom"
        : "inherit";

  return {
    customCidrCount: mapping.visibility?.custom_cidrs?.length ?? 0,
    regionCount: mapping.visibility?.selections?.length ?? 0,
    visibility,
    waf: !excluded && globalWafEnabled && mapping.waf_enabled !== false,
  };
};

export const getMappingDisplayTitle = (mapping: HostMapping): string =>
  mapping.title_override.trim() || mapping.title.trim();

export const getMappingFaviconSrc = (mapping: HostMapping): string => {
  const favicon = mapping.favicon_override?.trim() || mapping.favicon.trim();
  return /^data:image\//i.test(favicon) ? favicon : "";
};

export const getMappingFaviconSource = (
  mapping: HostMapping,
): "custom" | "auto" | "missing" => {
  if (/^data:image\//i.test(mapping.favicon_override?.trim() || "")) {
    return "custom";
  }
  return /^data:image\//i.test(mapping.favicon.trim()) ? "auto" : "missing";
};

export const getFaviconKey = (mapping: HostMapping): string =>
  `${mapping.host}::${getMappingFaviconSrc(mapping)}`;

export const normalizeMappingForm = (
  input: HostMapping,
  {
    hasFreshFaviconMetadata,
    hasFreshTitleMetadata,
    host,
    isAuthServiceTarget,
    isWebSocketTarget,
  }: {
    hasFreshFaviconMetadata: boolean;
    hasFreshTitleMetadata: boolean;
    host: string;
    isAuthServiceTarget: (target: string) => boolean;
    isWebSocketTarget: (target: string) => boolean;
  },
): HostMapping => {
  const targetType = normalizeHostMappingTargetType(input.target_type);
  const normalizedTarget = targetType === "proxy" ? input.target.trim() : "";
  const serviceRole =
    targetType === "proxy" && isAuthServiceTarget(normalizedTarget)
      ? "auth"
      : "app";
  const basicAuth =
    serviceRole === "auth" || targetType !== "proxy"
      ? createDisabledMappingBasicAuth()
      : normalizeMappingBasicAuth(input.basic_auth);

  return {
    host,
    group_id: serviceRole === "auth" ? null : input.group_id || null,
    target_type: targetType,
    target: normalizedTarget,
    static_serve:
      targetType === "proxy"
        ? null
        : normalizeHostMappingStaticServe(
            targetType,
            input.static_serve ?? createDefaultStaticServe(targetType),
          ),
    target_path_mode:
      serviceRole !== "auth" &&
      targetType === "proxy" &&
      input.target_path_mode === "prefix"
        ? "prefix"
        : DEFAULT_TARGET_PATH_MODE,
    waf_enabled: serviceRole === "auth" ? true : input.waf_enabled !== false,
    use_auth: serviceRole === "auth" ? false : input.use_auth,
    access_mode:
      serviceRole === "auth"
        ? DEFAULT_ACCESS_MODE
        : input.access_mode || DEFAULT_ACCESS_MODE,
    suppress_toolbar:
      targetType !== "proxy"
        ? true
        : serviceRole === "auth"
          ? false
          : isWebSocketTarget(normalizedTarget)
            ? true
            : input.suppress_toolbar,
    preserve_host: targetType === "proxy" && input.preserve_host === true,
    is_default: serviceRole === "auth" ? false : input.is_default === true,
    disabled: serviceRole === "auth" ? false : input.disabled === true,
    availability:
      serviceRole === "auth"
        ? null
        : normalizeHostMappingAvailability(input.availability),
    visibility:
      serviceRole === "auth"
        ? createDefaultMappingVisibility()
        : normalizeMappingVisibility(input.visibility),
    protocol_mode:
      input.protocol_mode === "http1" || input.protocol_mode === "http2"
        ? input.protocol_mode
        : DEFAULT_PROTOCOL_MODE,
    basic_auth: basicAuth.enabled
      ? basicAuth
      : createDisabledMappingBasicAuth(),
    locations:
      serviceRole === "auth" || targetType !== "proxy"
        ? []
        : [...(input.locations ?? [])],
    service_role: serviceRole,
    title:
      targetType === "proxy" && hasFreshTitleMetadata ? input.title.trim() : "",
    title_override: input.title_override.trim(),
    favicon:
      targetType === "proxy" && hasFreshFaviconMetadata
        ? input.favicon.trim()
        : "",
    favicon_override:
      serviceRole === "auth" ? "" : input.favicon_override?.trim() || "",
  };
};
