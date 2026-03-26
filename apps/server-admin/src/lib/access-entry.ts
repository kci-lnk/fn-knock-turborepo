import type { AppConfig } from "./redis";
import { resolveFrpcRemotePort } from "./frpc-config";
import { isReverseProxySubdomainMode } from "./reverse-proxy-submode";

export type AccessEntryInfo = {
  env: "GO_REPROXY_PORT" | "FRP_REMOTE_PORT";
  port: string;
  isDefault: boolean;
};

const resolveLocalGatewayPort = (): AccessEntryInfo => {
  const goReproxyPort = process.env.GO_REPROXY_PORT || "7999";
  return {
    env: "GO_REPROXY_PORT",
    port: goReproxyPort,
    isDefault: !process.env.GO_REPROXY_PORT,
  };
};

export const resolveAccessEntryInfo = (
  config?: Partial<
    Pick<AppConfig, "run_type" | "reverse_proxy_submode">
  > | null,
): AccessEntryInfo => {
  if (isReverseProxySubdomainMode(config)) {
    const frpRemotePort = resolveFrpcRemotePort();
    if (frpRemotePort) {
      return {
        env: "FRP_REMOTE_PORT",
        port: String(frpRemotePort),
        isDefault: false,
      };
    }
  }

  return resolveLocalGatewayPort();
};

export const resolvePublicGatewayPort = (
  config?: Partial<
    Pick<AppConfig, "run_type" | "reverse_proxy_submode">
  > | null,
): number | null => {
  const parsed = Number.parseInt(resolveAccessEntryInfo(config).port, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) return null;
  return parsed;
};
