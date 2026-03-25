export type ReverseProxySubmode = "path" | "subdomain";

export const DEFAULT_REVERSE_PROXY_SUBMODE: ReverseProxySubmode = "path";

export const normalizeReverseProxySubmode = (
  value: unknown,
): ReverseProxySubmode =>
  value === "subdomain" ? "subdomain" : DEFAULT_REVERSE_PROXY_SUBMODE;

export const resolveReverseProxySubmode = (
  config?: { reverse_proxy_submode?: unknown } | null,
): ReverseProxySubmode =>
  normalizeReverseProxySubmode(config?.reverse_proxy_submode);

export const isReverseProxySubdomainMode = (
  config?: { run_type?: 0 | 1 | 3; reverse_proxy_submode?: unknown } | null,
): boolean =>
  config?.run_type === 1 && resolveReverseProxySubmode(config) === "subdomain";

export const isAnySubdomainRoutingMode = (
  config?: { run_type?: 0 | 1 | 3; reverse_proxy_submode?: unknown } | null,
): boolean => config?.run_type === 3 || isReverseProxySubdomainMode(config);
