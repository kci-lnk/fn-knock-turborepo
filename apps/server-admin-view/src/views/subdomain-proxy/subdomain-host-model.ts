import type {
  DiscoveredServiceInfo,
  ScanDiscoverResponse,
} from "@/lib/api/scan";
import type { HostMapping, SubdomainModeConfig } from "@/types";
import {
  DEFAULT_ACCESS_MODE,
  DEFAULT_PROTOCOL_MODE,
  DEFAULT_TARGET_PATH_MODE,
  type DeleteDialogCopy,
  type DeleteDialogState,
  type DiscoveredHostResponse,
  type DiscoveredHostService,
  type EdgeClientIpProvider,
  type MappingInputMode,
} from "./subdomain-model-types";
import {
  createDefaultMappingVisibility,
  createDisabledMappingBasicAuth,
} from "./subdomain-mapping-model";

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
