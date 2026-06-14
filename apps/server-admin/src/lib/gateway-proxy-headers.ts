import { isAuthServiceTarget } from "./auth-service";
import { goBackend } from "./go-backend";
import {
  configManager,
  DEFAULT_GATEWAY_PROXY_HEADERS_CONFIG,
  type AppConfig,
  type GatewayProxyHeadersConfig,
  type GatewayProxyHeadersRuntimeState,
  type HostMapping,
} from "./redis";
import { isAnySubdomainRoutingMode } from "./reverse-proxy-submode";
import { tDefault } from "./i18n";

const gatewayProxyHeadersT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.gatewayProxyHeaders.${key}`, params);

export interface GatewayProxyHeadersItem {
  host: string;
  target: string;
  title: string;
  send_proxy_headers: boolean;
}

export interface GatewayProxyHeadersAvailability {
  available: boolean;
  reason: string;
}

export interface GatewayProxyHeadersSummary {
  total_count: number;
  disabled_count: number;
  updated_at: string | null;
}

export interface GatewayProxyHeadersDetails {
  config: GatewayProxyHeadersConfig;
  availability: GatewayProxyHeadersAvailability;
  items: GatewayProxyHeadersItem[];
  summary: GatewayProxyHeadersSummary;
}

const normalizeHostLike = (value: string | undefined | null): string =>
  String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/\/.*$/, "")
    .replace(/\.+$/, "");

const getRunTypeLabel = (runType: 0 | 1 | 3) => {
  if (runType === 0) return gatewayProxyHeadersT("runTypes.direct");
  if (runType === 1) return gatewayProxyHeadersT("runTypes.reverseProxy");
  return gatewayProxyHeadersT("runTypes.subdomain");
};

const getVisibleHostMappings = (
  config: Pick<AppConfig, "host_mappings">,
): HostMapping[] =>
  config.host_mappings.filter(
    (mapping) => !isAuthServiceTarget(mapping.target),
  );

const sanitizeDisabledHosts = (
  config: Pick<AppConfig, "host_mappings">,
  proxyHeadersConfig?: Partial<GatewayProxyHeadersConfig> | null,
): string[] => {
  const visibleHosts = new Set(
    getVisibleHostMappings(config).map((mapping) =>
      normalizeHostLike(mapping.host),
    ),
  );
  const rawHosts = Array.isArray(proxyHeadersConfig?.disabled_hosts)
    ? proxyHeadersConfig.disabled_hosts
    : [];
  const disabledHosts: string[] = [];
  const seen = new Set<string>();

  for (const rawHost of rawHosts) {
    const host = normalizeHostLike(rawHost);
    if (!host || seen.has(host) || !visibleHosts.has(host)) {
      continue;
    }
    seen.add(host);
    disabledHosts.push(host);
  }

  return disabledHosts;
};

export const buildGatewayProxyHeadersAvailability = (
  config: Pick<AppConfig, "run_type" | "reverse_proxy_submode">,
): GatewayProxyHeadersAvailability => {
  if (isAnySubdomainRoutingMode(config)) {
    return {
      available: true,
      reason: "",
    };
  }

  return {
    available: false,
    reason: gatewayProxyHeadersT("unavailableReason", {
      mode: getRunTypeLabel(config.run_type),
    }),
  };
};

export const buildGatewayProxyHeadersItems = (
  config: Pick<AppConfig, "host_mappings">,
  proxyHeadersConfig?: Partial<GatewayProxyHeadersConfig> | null,
): GatewayProxyHeadersItem[] => {
  const disabledHosts = new Set(
    sanitizeDisabledHosts(config, proxyHeadersConfig),
  );

  return getVisibleHostMappings(config).map((mapping) => ({
    host: mapping.host,
    target: mapping.target.trim(),
    title: mapping.title.trim(),
    send_proxy_headers: !disabledHosts.has(normalizeHostLike(mapping.host)),
  }));
};

export const buildGatewayProxyHeadersSummary = (
  items: GatewayProxyHeadersItem[],
  runtime: Pick<GatewayProxyHeadersRuntimeState, "updated_at">,
): GatewayProxyHeadersSummary => ({
  total_count: items.length,
  disabled_count: items.filter((item) => item.send_proxy_headers === false)
    .length,
  updated_at: runtime.updated_at,
});

export const compileGatewayProxyHeadersState = (
  config: Pick<
    AppConfig,
    | "run_type"
    | "reverse_proxy_submode"
    | "host_mappings"
    | "gateway_proxy_headers"
  >,
  proxyHeadersConfig?: Partial<GatewayProxyHeadersConfig> | null,
): {
  config: GatewayProxyHeadersConfig;
  runtime: GatewayProxyHeadersRuntimeState;
  items: GatewayProxyHeadersItem[];
} => {
  const nextConfig: GatewayProxyHeadersConfig = {
    disabled_hosts: sanitizeDisabledHosts(
      config,
      proxyHeadersConfig ??
        config.gateway_proxy_headers ??
        DEFAULT_GATEWAY_PROXY_HEADERS_CONFIG,
    ),
  };
  const items = buildGatewayProxyHeadersItems(config, nextConfig);
  const omitTargets: string[] = [];
  const seenTargets = new Set<string>();

  for (const item of items) {
    if (item.send_proxy_headers) continue;
    const target = item.target.trim();
    if (!target || seenTargets.has(target)) continue;
    seenTargets.add(target);
    omitTargets.push(target);
  }

  return {
    config: nextConfig,
    runtime: {
      enabled: isAnySubdomainRoutingMode(config),
      omit_targets: isAnySubdomainRoutingMode(config) ? omitTargets : [],
      updated_at: new Date().toISOString(),
    },
    items,
  };
};

export const getGatewayProxyHeadersDetails =
  async (): Promise<GatewayProxyHeadersDetails> => {
    const [config, proxyHeadersConfig, runtime] = await Promise.all([
      configManager.getConfig(),
      configManager.getGatewayProxyHeadersConfig(),
      configManager.getGatewayProxyHeadersRuntimeState(),
    ]);
    const nextConfig: GatewayProxyHeadersConfig = {
      disabled_hosts: sanitizeDisabledHosts(config, proxyHeadersConfig),
    };
    const items = buildGatewayProxyHeadersItems(config, nextConfig);

    return {
      config: nextConfig,
      availability: buildGatewayProxyHeadersAvailability(config),
      items,
      summary: buildGatewayProxyHeadersSummary(items, runtime),
    };
  };

export const syncGatewayProxyHeadersToGateway = async (
  runtime?: GatewayProxyHeadersRuntimeState | null,
): Promise<GatewayProxyHeadersRuntimeState> => {
  const nextRuntime =
    runtime ?? (await configManager.getGatewayProxyHeadersRuntimeState());
  const response = await goBackend.setForwardedHeadersConfig(nextRuntime);

  if (!response.success) {
    throw new Error(response.message || gatewayProxyHeadersT("syncFailed"));
  }

  return nextRuntime;
};

export const syncGatewayProxyHeadersRuntimeForConfig = async (
  config: Pick<
    AppConfig,
    | "run_type"
    | "reverse_proxy_submode"
    | "host_mappings"
    | "gateway_proxy_headers"
  >,
  options: {
    saveConfig?: boolean;
  } = {},
): Promise<{
  config: GatewayProxyHeadersConfig;
  runtime: GatewayProxyHeadersRuntimeState;
}> => {
  const compiled = compileGatewayProxyHeadersState(config);

  if (options.saveConfig === true) {
    await configManager.updateGatewayProxyHeadersConfig(compiled.config);
  }

  const runtime = await configManager.saveGatewayProxyHeadersRuntimeState(
    compiled.runtime,
  );
  await syncGatewayProxyHeadersToGateway(runtime);

  return {
    config: compiled.config,
    runtime,
  };
};
