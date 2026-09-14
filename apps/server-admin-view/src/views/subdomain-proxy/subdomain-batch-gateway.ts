import { ConfigAPI } from "@/lib/api/config";
import type { BatchGatewayKind } from "./useSubdomainBatchEdit";

export const createBatchGatewayAccess = (
  onUpdated: (kind: BatchGatewayKind, hosts: string[]) => void,
) => ({
  readGatewayHosts: async (kind: BatchGatewayKind) => {
    const details =
      kind === "gateway_proxy_headers"
        ? await ConfigAPI.getGatewayProxyHeaders()
        : await ConfigAPI.getGatewayHostResponse();
    return details.config.disabled_hosts;
  },
  writeGatewayHosts: async (kind: BatchGatewayKind, hosts: string[]) => {
    const details =
      kind === "gateway_proxy_headers"
        ? await ConfigAPI.updateGatewayProxyHeaders({ disabled_hosts: hosts })
        : await ConfigAPI.updateGatewayHostResponse({ disabled_hosts: hosts });
    onUpdated(kind, [...details.config.disabled_hosts]);
  },
});
