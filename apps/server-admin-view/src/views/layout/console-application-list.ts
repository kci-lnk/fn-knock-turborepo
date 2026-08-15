import type { AppConfig, DeploymentTarget, HostMapping } from "@/types";
import {
  isAnySubdomainRoutingMode,
  isReverseProxySubdomainMode,
  shouldOmitPublicAccessEntryPort,
} from "@/lib/reverse-proxy-submode";

export interface ConsoleApplicationItem {
  href: string;
  iconSrc: string;
  key: string;
  kind: "host" | "path";
  label: string;
  showIcon: boolean;
}

export interface ConsoleApplicationLocation {
  hostname: string;
  protocol: string;
}

const DEFAULT_GATEWAY_PORT = "7999";
type GatewayProtocol = "http:" | "https:";

export const isConsoleApplicationListAvailable = (
  deploymentTarget: DeploymentTarget | null | undefined,
): boolean => deploymentTarget === "fpk" || deploymentTarget === "fpk-lite";

export const shouldShowConsoleApplicationList = ({
  deploymentTarget,
  enabled,
}: {
  deploymentTarget: DeploymentTarget | null | undefined;
  enabled: unknown;
}): boolean =>
  isConsoleApplicationListAvailable(deploymentTarget) && enabled === true;

const normalizeGatewayPort = (value: unknown): string | null => {
  const normalized = String(value ?? "").trim();
  if (!normalized) return null;
  if (!/^\d{1,5}$/u.test(normalized)) return null;
  const parsed = Number(normalized);
  return Number.isInteger(parsed) && parsed >= 1 && parsed <= 65_535
    ? String(parsed)
    : null;
};

const normalizeGatewayProtocol = (value: string): GatewayProtocol =>
  value === "https:" ? "https:" : "http:";

const isDefaultGatewayPort = (
  protocol: GatewayProtocol,
  port: string,
): boolean =>
  (protocol === "https:" && port === "443") ||
  (protocol === "http:" && port === "80");

const resolvePublicBaseUrlPort = (
  config: AppConfig,
  protocol: GatewayProtocol,
): string | null => {
  const rawUrl = config.subdomain_mode?.public_auth_base_url?.trim();
  if (!rawUrl) return null;
  try {
    const parsed = new URL(rawUrl);
    if (parsed.protocol !== protocol) return null;
    const authority = rawUrl.match(/^[a-z][a-z\d+.-]*:\/\/([^/?#]*)/iu)?.[1];
    const explicitPort = authority?.match(/:(\d+)$/u)?.[1];
    return normalizeGatewayPort(explicitPort);
  } catch {
    return null;
  }
};

const resolveConfiguredPublicPort = (
  config: AppConfig,
  protocol: GatewayProtocol,
): string | null => {
  // Reverse-proxy subdomain mode publishes through the FRP access entry. Its
  // configured origin port must not replace the remote port returned by the
  // access-entry API.
  if (isReverseProxySubdomainMode(config)) return null;
  return normalizeGatewayPort(
    protocol === "https:"
      ? config.subdomain_mode?.public_https_port
      : config.subdomain_mode?.public_http_port,
  );
};

export const resolveConsoleApplicationGateway = (
  config: AppConfig,
  locationProtocol: string,
  accessEntryPort: string,
): { port: string; protocol: GatewayProtocol } => {
  const protocol = normalizeGatewayProtocol(locationProtocol);
  if (shouldOmitPublicAccessEntryPort(config)) {
    return { port: "", protocol };
  }

  const port =
    resolvePublicBaseUrlPort(config, protocol) ??
    resolveConfiguredPublicPort(config, protocol) ??
    normalizeGatewayPort(accessEntryPort) ??
    DEFAULT_GATEWAY_PORT;
  return {
    port: isDefaultGatewayPort(protocol, port) ? "" : port,
    protocol,
  };
};

const createGatewayUrl = (
  hostname: string,
  locationProtocol: string,
  config: AppConfig,
  accessEntryPort: string,
): URL | null => {
  try {
    const normalizedHostname = hostname.trim();
    if (!normalizedHostname) return null;
    const gateway = resolveConsoleApplicationGateway(
      config,
      locationProtocol,
      accessEntryPort,
    );
    const url = new URL(`${gateway.protocol}//${normalizedHostname}`);
    if (
      url.username ||
      url.password ||
      url.pathname !== "/" ||
      url.search ||
      url.hash
    ) {
      return null;
    }
    url.port = gateway.port;
    return url;
  } catch {
    return null;
  }
};

export const buildConsoleHostApplicationHref = (
  host: string,
  location: ConsoleApplicationLocation,
  config: AppConfig,
  accessEntryPort: string,
): string =>
  createGatewayUrl(host.trim(), location.protocol, config, accessEntryPort)
    ?.href ?? "";

export const buildConsolePathApplicationHref = (
  path: string,
  location: ConsoleApplicationLocation,
  config: AppConfig,
  accessEntryPort: string,
): string => {
  const url = createGatewayUrl(
    location.hostname,
    location.protocol,
    config,
    accessEntryPort,
  );
  if (!url) return "";

  const trimmedPath = path.trim();
  const withLeadingSlash = trimmedPath.startsWith("/")
    ? trimmedPath
    : `/${trimmedPath}`;
  url.pathname = withLeadingSlash.endsWith("/")
    ? withLeadingSlash
    : `${withLeadingSlash}/`;
  return url.href;
};

const normalizeText = (value: unknown): string =>
  typeof value === "string" ? value.trim() : "";

const getEligibleHostMappings = (config: AppConfig): HostMapping[] => {
  if (!isAnySubdomainRoutingMode(config)) return [];
  return config.host_mappings.filter(
    (mapping) =>
      mapping.service_role !== "auth" &&
      mapping.disabled !== true &&
      normalizeText(mapping.host).length > 0,
  );
};

const orderHostMappings = (
  mappings: HostMapping[],
  config: AppConfig,
): HostMapping[] => {
  if (
    config.host_mapping_grouped_view !== true ||
    config.host_mapping_groups.length === 0
  ) {
    return mappings;
  }

  const validGroupIds = new Set(
    config.host_mapping_groups.map((group) => group.id),
  );
  const ordered = config.host_mapping_groups.flatMap((group) =>
    mappings.filter((mapping) => mapping.group_id === group.id),
  );
  ordered.push(
    ...mappings.filter(
      (mapping) =>
        mapping.group_id == null || !validGroupIds.has(mapping.group_id),
    ),
  );
  return ordered;
};

const resolveHostLabel = (mapping: HostMapping, config: AppConfig): string => {
  if (config.gateway_portal?.display_style === "domain") {
    return normalizeText(mapping.host);
  }
  return (
    normalizeText(mapping.title_override) ||
    normalizeText(mapping.title) ||
    normalizeText(mapping.host)
  );
};

const resolveHostIcon = (mapping: HostMapping, config: AppConfig): string => {
  if (config.gateway_portal?.show_app_icon === false) return "";
  const icon =
    normalizeText(mapping.favicon_override) || normalizeText(mapping.favicon);
  return /^data:image\//i.test(icon) ? icon : "";
};

export const buildConsoleApplicationItems = ({
  accessEntryPort,
  config,
  location,
}: {
  accessEntryPort: string;
  config: AppConfig;
  location: ConsoleApplicationLocation;
}): ConsoleApplicationItem[] => {
  const hostMappings = orderHostMappings(
    getEligibleHostMappings(config),
    config,
  );
  if (hostMappings.length > 0) {
    const hostItems = hostMappings.flatMap((mapping) => {
      const href = buildConsoleHostApplicationHref(
        normalizeText(mapping.host),
        location,
        config,
        accessEntryPort,
      );
      if (!href) return [];
      return [
        {
          href,
          iconSrc: resolveHostIcon(mapping, config),
          key: `host:${mapping.host}`,
          kind: "host" as const,
          label: resolveHostLabel(mapping, config),
          showIcon: config.gateway_portal?.show_app_icon !== false,
        },
      ];
    });
    if (hostItems.length > 0) return hostItems;
  }

  return config.proxy_mappings.flatMap((mapping) => {
    const path = normalizeText(mapping.path);
    if (!path) return [];
    const href = buildConsolePathApplicationHref(
      path,
      location,
      config,
      accessEntryPort,
    );
    if (!href) return [];
    return [
      {
        href,
        iconSrc: "",
        key: `path:${path}`,
        kind: "path" as const,
        label: path,
        showIcon: true,
      },
    ];
  });
};
