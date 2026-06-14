import { isAuthServiceTarget } from "./auth-service";
import { goBackend } from "./go-backend";
import { tDefault, tWithFallback } from "./i18n";
import {
  configManager,
  DEFAULT_GATEWAY_HOST_RESPONSE_CONFIG,
  type AppConfig,
  type GatewayHostResponseConfig,
  type GatewayHostResponseRuntimeState,
  type HostMapping,
} from "./redis";
import { isAnySubdomainRoutingMode } from "./reverse-proxy-submode";
import {
  DEFAULT_LOCALE,
  type LocaleCode,
} from "../../../../packages/i18n/src";

export interface GatewayHostResponseItem {
  host: string;
  target: string;
  title: string;
  preserve_host: boolean;
}

export interface GatewayHostResponseAvailability {
  available: boolean;
  reason: string;
}

export interface GatewayHostResponseSummary {
  total_count: number;
  disabled_count: number;
  updated_at: string | null;
}

export interface GatewayHostResponseDetails {
  config: GatewayHostResponseConfig;
  availability: GatewayHostResponseAvailability;
  items: GatewayHostResponseItem[];
  summary: GatewayHostResponseSummary;
}

const normalizeHostLike = (value: string | undefined | null): string =>
  String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/\/.*$/, "")
    .replace(/\.+$/, "");

const getRunTypeLabel = (
  runType: 0 | 1 | 3,
  locale: LocaleCode = DEFAULT_LOCALE,
) => {
  if (runType === 0) {
    return tWithFallback(
      locale,
      "server.gatewayHostResponse.runTypes.direct",
      "Direct mode",
    );
  }
  if (runType === 1) {
    return tWithFallback(
      locale,
      "server.gatewayHostResponse.runTypes.reverseProxy",
      "Reverse proxy mode",
    );
  }
  return tWithFallback(
    locale,
    "server.gatewayHostResponse.runTypes.subdomain",
    "Subdomain mode",
  );
};

const getVisibleHostMappings = (
  config: Pick<AppConfig, "host_mappings">,
): HostMapping[] =>
  config.host_mappings.filter(
    (mapping) => !isAuthServiceTarget(mapping.target),
  );

const sanitizeDisabledHosts = (
  config: Pick<AppConfig, "host_mappings">,
  hostResponseConfig?: Partial<GatewayHostResponseConfig> | null,
): string[] => {
  const visibleHosts = new Set(
    getVisibleHostMappings(config).map((mapping) =>
      normalizeHostLike(mapping.host),
    ),
  );
  const rawHosts = Array.isArray(hostResponseConfig?.disabled_hosts)
    ? hostResponseConfig.disabled_hosts
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

export const buildGatewayHostResponseAvailability = (
  config: Pick<AppConfig, "run_type" | "reverse_proxy_submode">,
  locale: LocaleCode = DEFAULT_LOCALE,
): GatewayHostResponseAvailability => {
  if (isAnySubdomainRoutingMode(config)) {
    return {
      available: true,
      reason: "",
    };
  }

  return {
    available: false,
    reason: tWithFallback(
      locale,
      "server.gatewayHostResponse.unavailableReason",
      "Only subdomain mode is available. Current mode: {mode}.",
      { mode: getRunTypeLabel(config.run_type, locale) },
    ),
  };
};

export const buildGatewayHostResponseItems = (
  config: Pick<AppConfig, "host_mappings">,
  hostResponseConfig?: Partial<GatewayHostResponseConfig> | null,
): GatewayHostResponseItem[] => {
  const disabledHosts = new Set(
    sanitizeDisabledHosts(config, hostResponseConfig),
  );

  return getVisibleHostMappings(config).map((mapping) => ({
    host: mapping.host,
    target: mapping.target.trim(),
    title: mapping.title.trim(),
    preserve_host: !disabledHosts.has(normalizeHostLike(mapping.host)),
  }));
};

export const buildGatewayHostResponseSummary = (
  items: GatewayHostResponseItem[],
  runtime: Pick<GatewayHostResponseRuntimeState, "updated_at">,
): GatewayHostResponseSummary => ({
  total_count: items.length,
  disabled_count: items.filter((item) => item.preserve_host === false).length,
  updated_at: runtime.updated_at,
});

export const compileGatewayHostResponseState = (
  config: Pick<
    AppConfig,
    | "run_type"
    | "reverse_proxy_submode"
    | "host_mappings"
    | "gateway_host_response"
  >,
  hostResponseConfig?: Partial<GatewayHostResponseConfig> | null,
): {
  config: GatewayHostResponseConfig;
  runtime: GatewayHostResponseRuntimeState;
  items: GatewayHostResponseItem[];
} => {
  const nextConfig: GatewayHostResponseConfig = {
    disabled_hosts: sanitizeDisabledHosts(
      config,
      hostResponseConfig ??
        config.gateway_host_response ??
        DEFAULT_GATEWAY_HOST_RESPONSE_CONFIG,
    ),
  };
  const items = buildGatewayHostResponseItems(config, nextConfig);
  const omitTargets: string[] = [];
  const seenTargets = new Set<string>();

  for (const item of items) {
    if (item.preserve_host) continue;
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

export const getGatewayHostResponseDetails =
  async (
    locale: LocaleCode = DEFAULT_LOCALE,
  ): Promise<GatewayHostResponseDetails> => {
    const [config, hostResponseConfig, runtime] = await Promise.all([
      configManager.getConfig(),
      configManager.getGatewayHostResponseConfig(),
      configManager.getGatewayHostResponseRuntimeState(),
    ]);
    const nextConfig: GatewayHostResponseConfig = {
      disabled_hosts: sanitizeDisabledHosts(config, hostResponseConfig),
    };
    const items = buildGatewayHostResponseItems(config, nextConfig);

    return {
      config: nextConfig,
      availability: buildGatewayHostResponseAvailability(config, locale),
      items,
      summary: buildGatewayHostResponseSummary(items, runtime),
    };
  };

export const syncGatewayHostResponseToGateway = async (
  runtime?: GatewayHostResponseRuntimeState | null,
): Promise<GatewayHostResponseRuntimeState> => {
  const nextRuntime =
    runtime ?? (await configManager.getGatewayHostResponseRuntimeState());
  const response = await goBackend.setPreserveHostConfig(nextRuntime);

  if (!response.success) {
    throw new Error(
      response.message || tDefault("server.gatewayHostResponse.syncFailed"),
    );
  }

  return nextRuntime;
};

export const syncGatewayHostResponseRuntimeForConfig = async (
  config: Pick<
    AppConfig,
    | "run_type"
    | "reverse_proxy_submode"
    | "host_mappings"
    | "gateway_host_response"
  >,
  options: {
    saveConfig?: boolean;
  } = {},
): Promise<{
  config: GatewayHostResponseConfig;
  runtime: GatewayHostResponseRuntimeState;
}> => {
  const compiled = compileGatewayHostResponseState(config);

  if (options.saveConfig === true) {
    await configManager.updateGatewayHostResponseConfig(compiled.config);
  }

  const runtime = await configManager.saveGatewayHostResponseRuntimeState(
    compiled.runtime,
  );
  await syncGatewayHostResponseToGateway(runtime);
  if (isAnySubdomainRoutingMode(config)) {
    const response = await goBackend.setHostRules(config.host_mappings);
    if (!response.success) {
      throw new Error(
        response.message ||
          tDefault("server.gatewayHostResponse.hostRoutesSyncFailed"),
      );
    }
  }

  return {
    config: compiled.config,
    runtime,
  };
};
