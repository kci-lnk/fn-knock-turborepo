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

export const isCloudflaredReverseProxySubdomainMode = (
  config?:
    | Pick<AppConfig, "run_type" | "reverse_proxy_submode" | "default_tunnel">
    | null,
): boolean =>
  isReverseProxySubdomainMode(config) && config?.default_tunnel === "cloudflared";

export const shouldOmitPublicAccessEntryPort = (
  config?:
    | Pick<
        AppConfig,
        "run_type" | "reverse_proxy_submode" | "default_tunnel" | "subdomain_mode"
      >
    | null,
): boolean =>
  isCloudflaredReverseProxySubdomainMode(config) ||
  (config?.run_type === 3 &&
    config.subdomain_mode?.edge_client_ip_enabled === true &&
    (config.subdomain_mode?.aliyun_esa_enabled === true ||
      config.subdomain_mode?.tencent_edgeone_enabled === true));

const normalizePublicPort = (value: unknown): number | null => {
  const parsed =
    typeof value === "number"
      ? value
      : Number.parseInt(String(value ?? "").trim(), 10);
  if (!Number.isFinite(parsed) || parsed <= 0 || parsed > 65535) return null;
  return Math.floor(parsed);
};

const parsePublicBaseUrlPort = (
  rawUrl: string | undefined | null,
): number | null => {
  const raw = rawUrl?.trim();
  if (!raw) return null;

  try {
    return normalizePublicPort(new URL(raw).port);
  } catch {
    return null;
  }
};

export const resolveExplicitPublicAccessEntryPort = (
  config?: Pick<AppConfig, "subdomain_mode"> | null,
): number | null =>
  parsePublicBaseUrlPort(config?.subdomain_mode?.public_auth_base_url) ||
  normalizePublicPort(config?.subdomain_mode?.public_https_port) ||
  normalizePublicPort(config?.subdomain_mode?.public_http_port);

export const isCloudflaredTunnelAvailable = (
  config?: Pick<AppConfig, "run_type" | "reverse_proxy_submode"> | null,
): boolean => {
  if (config?.run_type !== 1) return false;
  const submode = resolveReverseProxySubmode(config);
  return submode === "path" || submode === "subdomain";
};
