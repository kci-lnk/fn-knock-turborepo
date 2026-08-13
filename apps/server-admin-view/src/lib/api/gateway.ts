import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";
import type {
  CidrCitiesPayload,
  CidrCapabilitiesPayload,
  CidrLookupPayload,
  CidrOperator,
  CidrProvincesPayload,
  CidrSelectorPayload,
  GatewayLogDatesPayload,
  GatewayLogAnalyticsPayload,
  GatewayLogDeletePayload,
  GatewayLogEntriesPayload,
  GatewayLoggingConfig,
  IpLocationBatchPayload,
  IpLocationSnapshot,
  WAFDetails,
  WAFDrainResult,
  WAFLogDeletePayload,
  WAFLogEntriesPayload,
  WAFRuleFileContent,
  WAFStatus,
} from "../../types";
import { apiClient } from "./client";

type GatewayLoggingConfigUpdate =
  ApiContractComponents["schemas"]["GatewayLoggingConfigUpdateData"];
type GatewayLogDirectory =
  ApiContractComponents["schemas"]["GatewayLogDirectoryData"];
type GatewayLogEntriesQuery = NonNullable<
  ApiContractOperations["get_api_admin_gateway_logs_entries"]["parameters"]["query"]
>;
type GatewayLogAnalyticsQuery = NonNullable<
  ApiContractOperations["get_api_admin_gateway_logs_analytics"]["parameters"]["query"]
>;
type GatewayLogAnalyticsRefresh =
  ApiContractComponents["schemas"]["GatewayLogAnalyticsRefreshData"];
type GatewayLogDeleteRequest =
  ApiContractComponents["schemas"]["GatewayLogDeleteBodyData"];
type IpLocationBatchBody =
  ApiContractComponents["schemas"]["IpLocationBatchBodyData"];
type CidrCitiesQuery = NonNullable<
  ApiContractOperations["get_api_admin_cidr_cities"]["parameters"]["query"]
>;
type CidrSelectorQuery = NonNullable<
  ApiContractOperations["get_api_admin_cidr_selector"]["parameters"]["query"]
>;
type CidrLookupQuery = NonNullable<
  ApiContractOperations["get_api_admin_cidr_cidrs"]["parameters"]["query"]
>;
type WafConfigUpdate = ApiContractComponents["schemas"]["WafConfigUpdateData"];
type WafRuleToggleBody =
  ApiContractComponents["schemas"]["WafRuleToggleBodyData"];
type WafUploadBody = ApiContractComponents["schemas"]["WafUploadBodyData"];
type WafLogQuery = NonNullable<
  ApiContractOperations["get_api_admin_waf_logs"]["parameters"]["query"]
>;
type WafLogDeleteBody =
  ApiContractComponents["schemas"]["WafLogDeleteBodyData"];

export { ConfigAPI } from "./config";
export { DashboardAPI } from "./dashboard";
export type {
  CidrCitiesPayload,
  CidrLookupPayload,
  CidrProvincesPayload,
  CidrSelectorPayload,
  GatewayHostResponseDetails,
  GatewayLogDatesPayload,
  GatewayLogAnalyticsPayload,
  GatewayLogDeletePayload,
  GatewayLogEntriesPayload,
  GatewayLoggingConfig,
  GatewayPortalConfig,
  GatewayProxyHeadersDetails,
  GatewayVisibilityDetails,
  IpLocationBatchPayload,
  IpLocationSnapshot,
  ProtocolMappingFeatureConfig,
  SmartConnectConfig,
  SmartConnectDetails,
  ThreatOverview,
  WAFConfig,
  WAFDetails,
  WAFDrainResult,
  WAFLogDeletePayload,
  WAFLogEntriesPayload,
  WAFRuleFileContent,
  WAFStatus,
} from "../../types";
export * from "./scan";

export const GatewayLogsAPI = {
  async getConfig(): Promise<GatewayLoggingConfig> {
    const res = await apiClient.get("/gateway-logs/config");
    return res.data.data;
  },
  async updateConfig(
    payload: GatewayLoggingConfigUpdate,
  ): Promise<GatewayLoggingConfig> {
    const res = await apiClient.post("/gateway-logs/config", payload);
    return res.data.data;
  },
  async getDirectory(): Promise<GatewayLogDirectory> {
    const res = await apiClient.get("/gateway-logs/directory");
    return res.data.data;
  },
  async getDates(): Promise<GatewayLogDatesPayload> {
    const res = await apiClient.get("/gateway-logs/dates");
    return res.data.data;
  },
  async getEntries(
    params: GatewayLogEntriesQuery,
  ): Promise<GatewayLogEntriesPayload> {
    const res = await apiClient.get("/gateway-logs/entries", {
      params,
    });
    return res.data.data;
  },
  async getAnalytics(
    params: GatewayLogAnalyticsQuery,
  ): Promise<GatewayLogAnalyticsPayload> {
    const res = await apiClient.get("/gateway-logs/analytics", { params });
    return res.data.data;
  },
  async refreshAnalyticsGeo(
    params: GatewayLogAnalyticsQuery,
  ): Promise<GatewayLogAnalyticsRefresh> {
    const res = await apiClient.post("/gateway-logs/analytics", undefined, {
      params,
    });
    return res.data.data;
  },
  async deleteDate(date: string): Promise<GatewayLogDeletePayload> {
    const res = await apiClient.delete("/gateway-logs/entries", {
      data: { date } satisfies GatewayLogDeleteRequest,
    });
    return res.data.data;
  },
};

export const WAFAPI = {
  async getDetails(): Promise<WAFDetails> {
    const res = await apiClient.get("/waf/details");
    return res.data.data;
  },
  async getStatus(): Promise<WAFStatus> {
    const res = await apiClient.get("/waf/status");
    return res.data.data;
  },
  async updateConfig(payload: WafConfigUpdate): Promise<WAFDetails> {
    const res = await apiClient.post("/waf/config", payload);
    return res.data.data;
  },
  async refreshManifest(): Promise<WAFDetails> {
    const res = await apiClient.post("/waf/manifest/refresh");
    return res.data.data;
  },
  async syncSystemRules(): Promise<WAFDetails> {
    const res = await apiClient.post("/waf/system/sync");
    return res.data.data;
  },
  async setRulesEnabled(payload: WafRuleToggleBody): Promise<WAFDetails> {
    const res = await apiClient.post("/waf/rules/enabled", payload);
    return res.data.data;
  },
  async enableRecommendedSystemRules(): Promise<WAFDetails> {
    const res = await apiClient.post("/waf/rules/recommended");
    return res.data.data;
  },
  async getRuleFile(
    source: "system" | "custom",
    filename: string,
  ): Promise<WAFRuleFileContent> {
    const res = await apiClient.get(
      `/waf/rules/${source}/${encodeURIComponent(filename)}`,
    );
    return res.data.data;
  },
  async uploadCustomRules(payload: WafUploadBody): Promise<WAFDetails> {
    const res = await apiClient.post("/waf/custom/upload", payload);
    return res.data.data;
  },
  async deleteCustomRule(filename: string): Promise<WAFDetails> {
    const res = await apiClient.delete(
      `/waf/custom/${encodeURIComponent(filename)}`,
    );
    return res.data.data;
  },
  async drainEvents(signal?: AbortSignal): Promise<WAFDrainResult> {
    const res = await apiClient.post("/waf/events/drain", undefined, {
      signal,
    });
    return res.data.data;
  },
  async getLogs(
    params: WafLogQuery,
    signal?: AbortSignal,
  ): Promise<WAFLogEntriesPayload> {
    const res = await apiClient.get("/waf/logs", { params, signal });
    return res.data.data;
  },
  async getLog(
    traceId: string,
  ): Promise<WAFLogEntriesPayload["items"][number]> {
    const res = await apiClient.get(`/waf/logs/${encodeURIComponent(traceId)}`);
    return res.data.data;
  },
  async deleteLogs(date: string): Promise<WAFLogDeletePayload> {
    const res = await apiClient.delete("/waf/logs", {
      data: { date } satisfies WafLogDeleteBody,
    });
    return res.data.data;
  },
};

const IP_LOCATION_BATCH_LIMIT = 20;

export const IpLocationAPI = {
  async lookupBatch(ips: string[]): Promise<IpLocationSnapshot[]> {
    if (ips.length === 0) return [];

    const tasks: Promise<IpLocationSnapshot[]>[] = [];
    for (let index = 0; index < ips.length; index += IP_LOCATION_BATCH_LIMIT) {
      const batch = ips.slice(index, index + IP_LOCATION_BATCH_LIMIT);
      const body = { ips: batch } satisfies IpLocationBatchBody;
      tasks.push(
        apiClient
          .post("/ip-location/batch", body)
          .then(
            (res) =>
              ((res.data.data as IpLocationBatchPayload).items ||
                []) as IpLocationSnapshot[],
          ),
      );
    }

    const groups = await Promise.all(tasks);
    return groups.flat();
  },
};

export const CidrAPI = {
  async getCapabilities(): Promise<CidrCapabilitiesPayload> {
    const res = await apiClient.get("/cidr/capabilities");
    return res.data.data;
  },
  async getProvinces(): Promise<CidrProvincesPayload> {
    const res = await apiClient.get("/cidr/provinces");
    return res.data.data;
  },
  async getCities(province: string): Promise<CidrCitiesPayload> {
    const params = { province } satisfies CidrCitiesQuery;
    const res = await apiClient.get("/cidr/cities", {
      params,
    });
    return res.data.data;
  },
  async getSelector(province?: string): Promise<CidrSelectorPayload> {
    const params = province
      ? ({ province } satisfies CidrSelectorQuery)
      : undefined;
    const res = await apiClient.get("/cidr/selector", {
      params,
    });
    return res.data.data;
  },
  async getCidrs(payload: {
    province: string;
    city?: string | null;
    operator?: CidrOperator | null;
  }): Promise<CidrLookupPayload> {
    const params = {
      province: payload.province,
      ...(payload.city ? { city: payload.city } : {}),
      ...(payload.operator ? { operator: payload.operator } : {}),
    } satisfies CidrLookupQuery;
    const res = await apiClient.get("/cidr/cidrs", { params });
    return res.data.data;
  },
};
