import type { AppConfig, ReverseProxySubmode } from "../types";

export const DEFAULT_REVERSE_PROXY_SUBMODE: ReverseProxySubmode = "path";

export const normalizeReverseProxySubmode = (
  value: unknown,
): ReverseProxySubmode =>
  value === "subdomain" ? "subdomain" : DEFAULT_REVERSE_PROXY_SUBMODE;

export const resolveReverseProxySubmode = (
  config?: Pick<AppConfig, "reverse_proxy_submode"> | null,
): ReverseProxySubmode =>
  normalizeReverseProxySubmode(config?.reverse_proxy_submode);

export const isReverseProxySubdomainMode = (
  config?: Pick<AppConfig, "run_type" | "reverse_proxy_submode"> | null,
): boolean =>
  config?.run_type === 1 && resolveReverseProxySubmode(config) === "subdomain";

export const isAnySubdomainRoutingMode = (
  config?: Pick<AppConfig, "run_type" | "reverse_proxy_submode"> | null,
): boolean => config?.run_type === 3 || isReverseProxySubdomainMode(config);

export const isCloudflaredTunnelAvailable = (
  config?: Pick<AppConfig, "run_type" | "reverse_proxy_submode"> | null,
): boolean =>
  config?.run_type === 1 && resolveReverseProxySubmode(config) === "path";
