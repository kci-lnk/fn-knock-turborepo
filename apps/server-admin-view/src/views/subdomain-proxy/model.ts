import { extractPortFromTarget } from "@admin-shared/utils/extractPortFromTarget";
import {
  isHttpProxyTargetProtocol,
  isSupportedProxyTargetUrl,
} from "@admin-shared/utils/proxyTargetInput";
import {
  type DiscoveredServiceInfo,
  type ScanDiscoverResponse,
} from "@/lib/api/scan";
import { normalizeHostMappingAvailability } from "@/lib/host-mapping-availability";
import type { HostMapping, SubdomainModeConfig } from "@/types";
export {
  formatHostMappingAvailabilityWindow,
  getAvailabilityWindowValidationError,
  getHostMappingAvailabilityState,
  isAvailabilityWindowOpen,
  isAvailabilityWindowValid,
  isHostMappingUnavailable,
  normalizeHostMappingAvailability,
  parseAvailabilityTimeToMinutes,
  type HostMappingAvailabilityState,
} from "@/lib/host-mapping-availability";

export type MappingInputMode = "subdomain" | "full_host";

export type DiscoveredHostService = DiscoveredServiceInfo & {
  suggestedSubdomain: string;
};

export type DiscoveredHostResponse = Omit<ScanDiscoverResponse, "services"> & {
  services: DiscoveredHostService[];
};

export type EdgeClientIpProvider = "aliyun_esa" | "tencent_edgeone";

export const DEFAULT_AUTH_SUBDOMAIN = "auth";
export const DEFAULT_ACCESS_MODE: HostMapping["access_mode"] = "login_first";
export const DEFAULT_PROTOCOL_MODE: HostMapping["protocol_mode"] = "auto";
export const DEFAULT_TARGET_PATH_MODE: HostMapping["target_path_mode"] =
  "entry";
export const HOME_ASSISTANT_TARGET_PORT = 8123;

export type DeleteDialogState =
  | {
      kind: "clear_all";
      step: 1 | 2;
    }
  | {
      kind: "mapping";
      host: string;
    };

export type TranslationParams = Record<string, string | number>;

export interface TranslationSpec {
  key: string;
  params?: TranslationParams;
}

export interface DeleteDialogCopy {
  title: TranslationSpec;
  description: TranslationSpec;
  confirmLabel: TranslationSpec;
}

export const buildDeleteDialogCopy = (
  target: DeleteDialogState,
  mappingsCount: number,
): DeleteDialogCopy => {
  if (target.kind === "clear_all") {
    return {
      title: {
        key:
          target.step === 1
            ? "admin.subdomainProxy.clearAllTitle"
            : "admin.subdomainProxy.clearAllSecondTitle",
      },
      description: {
        key:
          target.step === 1
            ? "admin.subdomainProxy.clearAllDescriptionFirst"
            : "admin.subdomainProxy.clearAllDescriptionSecond",
        params: target.step === 1 ? { count: mappingsCount } : undefined,
      },
      confirmLabel: {
        key:
          target.step === 1
            ? "admin.subdomainProxy.continueConfirm"
            : "admin.subdomainProxy.confirmClear",
      },
    };
  }

  return {
    title: { key: "admin.subdomainProxy.deleteMappingTitle" },
    description: {
      key: "admin.subdomainProxy.deleteMappingDescription",
      params: { host: target.host },
    },
    confirmLabel: { key: "admin.subdomainProxy.deleteMapping" },
  };
};

export const normalizeHostLike = (value: string): string => {
  const authority = value
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/\/.*$/, "")
    .replace(/\.+$/, "");
  if (authority.startsWith("[")) {
    const end = authority.indexOf("]");
    return end >= 0 ? authority.slice(0, end + 1) : authority;
  }
  const separator = authority.lastIndexOf(":");
  return separator >= 0 && !authority.slice(0, separator).includes(":")
    ? authority.slice(0, separator).replace(/\.+$/, "")
    : authority;
};

export const normalizeRootDomainValue = (value: string): string =>
  normalizeHostLike(value);

export const hasSubdomainRootDomainWildcard = (value: string): boolean =>
  value.includes("*");

export const stripRootDomainSuffix = (
  value: string,
  rootDomain: string,
): string => {
  const normalized = normalizeHostLike(value);
  const normalizedRoot = normalizeRootDomainValue(rootDomain);
  if (!normalizedRoot) return normalized;
  if (normalized === normalizedRoot) return "";
  if (normalized.endsWith(`.${normalizedRoot}`)) {
    return normalized.slice(0, -1 * (normalizedRoot.length + 1));
  }
  return normalized;
};

export const composeHostFromSubdomain = (
  subdomain: string,
  rootDomain: string,
): string => {
  if (hasSubdomainRootDomainWildcard(rootDomain)) return "";
  const normalizedRoot = normalizeRootDomainValue(rootDomain);
  const normalizedSubdomain = stripRootDomainSuffix(subdomain, normalizedRoot);
  if (!normalizedRoot || !normalizedSubdomain) return "";
  return `${normalizedSubdomain}.${normalizedRoot}`;
};

export const extractSubdomainFromHost = (
  value: string,
  rootDomain: string,
): string | null => {
  const normalizedHost = normalizeHostLike(value);
  const normalizedRoot = normalizeRootDomainValue(rootDomain);
  if (!normalizedHost || !normalizedRoot) return null;
  if (!normalizedHost.endsWith(`.${normalizedRoot}`)) return null;

  const subdomain = normalizedHost.slice(0, -1 * (normalizedRoot.length + 1));
  return subdomain || null;
};

export const resolveMappingEditorState = (
  host: string,
  rootDomain: string,
): { mode: MappingInputMode; value: string } => {
  const subdomain = extractSubdomainFromHost(host, rootDomain);
  if (subdomain) {
    return {
      mode: "subdomain",
      value: subdomain,
    };
  }

  return {
    mode: "full_host",
    value: normalizeHostLike(host),
  };
};

export const buildSuggestedSubdomain = (
  service: DiscoveredServiceInfo,
): string => {
  const candidates = [
    service.detail.rule.path,
    service.detail.label,
    service.detail.name,
    `app-${service.port}`,
  ];

  for (const candidate of candidates) {
    const normalized = String(candidate ?? "")
      .trim()
      .replace(/^\/+|\/+$/g, "")
      .replace(/\//g, "-")
      .replace(/\s+/g, "-")
      .replace(/[^a-zA-Z0-9-]+/g, "-")
      .replace(/-+/g, "-")
      .replace(/^-+|-+$/g, "")
      .toLowerCase();

    if (normalized) return normalized;
  }

  return `app-${service.port}`;
};

export const resolveDiscoveredServiceHost = (
  service: Pick<DiscoveredServiceInfo, "host">,
  fallbackHost: string | null | undefined,
) => service.host?.trim() || fallbackHost?.trim() || "127.0.0.1";

const normalizeEndpointHost = (value: string): string =>
  value.trim().replace(/^\[/, "").replace(/\]$/, "").toLowerCase();

const getDefaultTargetPort = (protocol: string): number | null => {
  if (protocol === "http:" || protocol === "ws:") return 80;
  if (protocol === "https:" || protocol === "wss:") return 443;
  return null;
};

const parseTargetUrl = (value: string): URL => {
  if (/^[a-z][a-z\d+.-]*:\/\//i.test(value)) {
    return new URL(value);
  }
  if (value.startsWith("//")) {
    return new URL(`http:${value}`);
  }
  return new URL(`http://${value}`);
};

export const buildDiscoveredServiceTargetKey = (
  service: Pick<DiscoveredServiceInfo, "host" | "port">,
  fallbackHost: string | null | undefined,
): string => {
  const host = normalizeEndpointHost(
    resolveDiscoveredServiceHost(service, fallbackHost),
  );
  return host && Number.isFinite(service.port) ? `${host}:${service.port}` : "";
};

export const buildMappingTargetKey = (target: string): string => {
  const normalizedTarget = target.trim();
  if (!normalizedTarget) return "";

  try {
    const parsed = parseTargetUrl(normalizedTarget);
    const port = parsed.port
      ? Number.parseInt(parsed.port, 10)
      : getDefaultTargetPort(parsed.protocol);
    const host = normalizeEndpointHost(parsed.hostname);
    return host && port !== null && Number.isFinite(port)
      ? `${host}:${port}`
      : "";
  } catch {
    return "";
  }
};

export const buildDiscoveredHostResponse = (
  data: ScanDiscoverResponse,
  existingTargets: Set<string>,
): DiscoveredHostResponse => ({
  ...data,
  services: data.services
    .map((service) => ({
      ...service,
      detail: {
        ...service.detail,
        rule: { ...service.detail.rule },
      },
      suggestedSubdomain: buildSuggestedSubdomain(service),
    }))
    .filter(
      (service) =>
        !existingTargets.has(
          buildDiscoveredServiceTargetKey(service, data.host),
        ),
    ),
});

export const collectDuplicateValues = (values: string[]): string[] => {
  const seen = new Set<string>();
  const duplicates = new Set<string>();
  for (const value of values) {
    if (!value) continue;
    if (seen.has(value)) {
      duplicates.add(value);
      continue;
    }
    seen.add(value);
  }
  return [...duplicates];
};

export const buildDiscoveredServiceMappings = ({
  fallbackHost,
  rootDomain,
  services,
}: {
  fallbackHost: string | null | undefined;
  rootDomain: string;
  services: DiscoveredHostService[];
}): HostMapping[] =>
  services.map((service) => ({
    host: composeHostFromSubdomain(service.suggestedSubdomain, rootDomain),
    group_id: null,
    target: `http://${resolveDiscoveredServiceHost(service, fallbackHost)}:${service.port}/`,
    target_path_mode: DEFAULT_TARGET_PATH_MODE,
    waf_enabled: true,
    use_auth: service.detail.rule.use_auth,
    access_mode: DEFAULT_ACCESS_MODE,
    suppress_toolbar: false,
    preserve_host: true,
    is_default: false,
    disabled: false,
    availability: null,
    protocol_mode: DEFAULT_PROTOCOL_MODE,
    basic_auth: createDisabledMappingBasicAuth(),
    visibility: createDefaultMappingVisibility(),
    locations: [],
    service_role: "app",
    title: "",
    title_override: "",
    favicon: "",
    favicon_override: "",
  }));

export const resolveEdgeClientIpProvider = (
  value: Pick<
    SubdomainModeConfig,
    "edge_client_ip_enabled" | "aliyun_esa_enabled" | "tencent_edgeone_enabled"
  >,
): EdgeClientIpProvider | null => {
  if (!value.edge_client_ip_enabled) return null;
  if (value.tencent_edgeone_enabled) return "tencent_edgeone";
  if (value.aliyun_esa_enabled) return "aliyun_esa";
  return null;
};

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

export const canRefreshHostMappingMetadata = (target: string): boolean => {
  const normalizedTarget = target.trim();
  if (!isSupportedProxyTargetUrl(normalizedTarget)) return false;

  try {
    const parsed = new URL(normalizedTarget);
    return isHttpProxyTargetProtocol(parsed.protocol);
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
  target,
}: {
  basicAuthValidationMessage: string;
  canUseRootDomainSuffix: boolean;
  host: string;
  inputMode: MappingInputMode;
  target: string;
}): boolean => {
  const normalizedTarget = target.trim();

  if (!host || !normalizedTarget) return false;
  if (inputMode === "subdomain" && !canUseRootDomainSuffix) {
    return false;
  }

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
    configuredHttpsPort ||
    configuredHttpPort
  );
};

export const resolveConfiguredAccessEntryPublicPort = (
  config: Pick<
    SubdomainModeConfig,
    "public_auth_base_url" | "public_http_port" | "public_https_port"
  >,
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
    configuredHttpsPort ||
    explicitHttpPort ||
    configuredHttpPort;
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
  target: "",
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

export const getLocationRulesCount = (mapping: HostMapping): number =>
  mapping.locations?.length ?? 0;

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
  const normalizedTarget = input.target.trim();
  const serviceRole = isAuthServiceTarget(normalizedTarget) ? "auth" : "app";
  const basicAuth =
    serviceRole === "auth"
      ? createDisabledMappingBasicAuth()
      : normalizeMappingBasicAuth(input.basic_auth);

  return {
    host,
    group_id: serviceRole === "auth" ? null : input.group_id || null,
    target: normalizedTarget,
    target_path_mode:
      serviceRole !== "auth" && input.target_path_mode === "prefix"
        ? "prefix"
        : DEFAULT_TARGET_PATH_MODE,
    waf_enabled: serviceRole === "auth" ? true : input.waf_enabled !== false,
    use_auth: serviceRole === "auth" ? false : input.use_auth,
    access_mode:
      serviceRole === "auth"
        ? DEFAULT_ACCESS_MODE
        : input.access_mode || DEFAULT_ACCESS_MODE,
    suppress_toolbar:
      serviceRole === "auth"
        ? false
        : isWebSocketTarget(normalizedTarget)
          ? true
          : input.suppress_toolbar,
    preserve_host: input.preserve_host === true,
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
    locations: serviceRole === "auth" ? [] : [...(input.locations ?? [])],
    service_role: serviceRole,
    title: hasFreshTitleMetadata ? input.title.trim() : "",
    title_override: input.title_override.trim(),
    favicon: hasFreshFaviconMetadata ? input.favicon.trim() : "",
    favicon_override:
      serviceRole === "auth" ? "" : input.favicon_override?.trim() || "",
  };
};

export const normalizeDisabledHosts = (
  hosts: string[] | undefined | null,
): string[] => [
  ...new Set((hosts ?? []).map(normalizeHostLike).filter(Boolean)),
];

export const hasSameDisabledHosts = (
  left: string[] | undefined | null,
  right: string[] | undefined | null,
): boolean => {
  const leftHosts = normalizeDisabledHosts(left);
  const rightHosts = normalizeDisabledHosts(right);
  return (
    leftHosts.length === rightHosts.length &&
    leftHosts.every((host, index) => host === rightHosts[index])
  );
};

export const mergeGatewayDisabledHostsForMapping = (
  currentDisabledHosts: string[],
  previousHosts: string[],
  nextHost: string,
  enabledForNextHost: boolean,
): string[] => {
  const disabledHosts = new Set(normalizeDisabledHosts(currentDisabledHosts));
  const normalizedNextHost = normalizeHostLike(nextHost);

  for (const host of normalizeDisabledHosts(previousHosts)) {
    disabledHosts.delete(host);
  }

  if (normalizedNextHost) {
    if (enabledForNextHost) {
      disabledHosts.delete(normalizedNextHost);
    } else {
      disabledHosts.add(normalizedNextHost);
    }
  }

  return [...disabledHosts];
};

export const hasSameMappingOrder = (
  left: HostMapping[],
  right: HostMapping[],
) =>
  left.length === right.length &&
  left.every((mapping, index) => mapping.host === right[index]?.host);

export const mergeFilteredMappingsOrder = ({
  allMappings,
  filteredMappings,
  isPinnedMapping,
  nextFiltered,
  visibleMappings,
}: {
  allMappings: HostMapping[];
  filteredMappings: HostMapping[];
  isPinnedMapping?: (mapping: HostMapping) => boolean;
  nextFiltered: HostMapping[];
  visibleMappings: HostMapping[];
}): HostMapping[] => {
  const filteredHostSet = new Set(filteredMappings.map((item) => item.host));
  let nextFilteredIndex = 0;
  const nextVisible = visibleMappings.map((mapping) =>
    filteredHostSet.has(mapping.host)
      ? (nextFiltered[nextFilteredIndex++] ?? mapping)
      : mapping,
  );

  let nextVisibleIndex = 0;
  return allMappings.map((mapping) =>
    (isPinnedMapping?.(mapping) ?? mapping.service_role === "auth")
      ? mapping
      : (nextVisible[nextVisibleIndex++] ?? mapping),
  );
};
