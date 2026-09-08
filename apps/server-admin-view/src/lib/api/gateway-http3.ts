import { apiClient } from "./client";

import type { components } from "@fn-knock/api-contract";

export type GatewayHttp3Config =
  components["schemas"]["GatewayHttp3UpdateData"];
export type GatewayHttp3Status = components["schemas"]["GatewayHttp3Data"];
export const gatewayHttp3Api = {
  async get(): Promise<GatewayHttp3Status> {
    return (await apiClient.get("/config/gateway/http3")).data.data;
  },
  async set(config: GatewayHttp3Config): Promise<GatewayHttp3Status> {
    return (
      await apiClient.post("/config/gateway/http3", config, { timeout: 45000 })
    ).data.data;
  },
};
