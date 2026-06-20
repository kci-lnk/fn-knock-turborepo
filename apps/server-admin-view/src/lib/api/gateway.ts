import type {
  CidrCitiesPayload,
  CidrLookupPayload,
  CidrProvincesPayload,
  CidrSelectorPayload,
  GatewayLogDatesPayload,
  GatewayLogDeletePayload,
  GatewayLogEntriesPayload,
  GatewayLoggingConfig,
  IpLocationBatchPayload,
  IpLocationSnapshot,
  WAFConfig,
  WAFDetails,
  WAFDrainResult,
  WAFLogDeletePayload,
  WAFLogEntriesPayload,
  WAFRuleFileContent,
  WAFStatus,
} from "../../types";
import { apiClient } from "./client";

export { ConfigAPI } from "./config";
export { DashboardAPI } from "./dashboard";
export type {
  CidrCitiesPayload,
  CidrLookupPayload,
  CidrProvincesPayload,
  CidrSelectorPayload,
  GatewayHostResponseDetails,
  GatewayLogDatesPayload,
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
    payload: Pick<GatewayLoggingConfig, "enabled" | "max_days">,
  ): Promise<GatewayLoggingConfig> {
    const res = await apiClient.post("/gateway-logs/config", payload);
    return res.data.data;
  },
  async getDirectory(): Promise<{ logs_dir: string }> {
    const res = await apiClient.get("/gateway-logs/directory");
    return res.data.data;
  },
  async getDates(): Promise<GatewayLogDatesPayload> {
    const res = await apiClient.get("/gateway-logs/dates");
    return res.data.data;
  },
  async getEntries(params: {
    date: string;
    pagination: "page" | "cursor";
    limit: string;
    cursor?: string;
    search?: string;
    status?: string;
    logged_in?: string;
    waf_status?: string;
    page?: number;
  }): Promise<GatewayLogEntriesPayload> {
    const res = await apiClient.get("/gateway-logs/entries", {
      params,
    });
    return res.data.data;
  },
  async deleteDate(date: string): Promise<GatewayLogDeletePayload> {
    const res = await apiClient.delete("/gateway-logs/entries", {
      data: { date },
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
  async updateConfig(
    payload: Partial<
      Pick<
        WAFConfig,
        | "enabled"
        | "system_rules_auto_update_enabled"
        | "common_location_exempt_enabled"
        | "paranoia_level"
        | "executing_paranoia_level"
      >
    >,
  ): Promise<WAFDetails> {
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
  async setRulesEnabled(payload: {
    source: "system" | "custom";
    filenames?: string[];
    enabled: boolean;
  }): Promise<WAFDetails> {
    const res = await apiClient.post("/waf/rules/enabled", payload);
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
  async uploadCustomRules(payload: {
    files: Array<{ filename: string; content_base64: string }>;
  }): Promise<WAFDetails> {
    const res = await apiClient.post("/waf/custom/upload", payload);
    return res.data.data;
  },
  async deleteCustomRule(filename: string): Promise<WAFDetails> {
    const res = await apiClient.delete(
      `/waf/custom/${encodeURIComponent(filename)}`,
    );
    return res.data.data;
  },
  async drainEvents(): Promise<WAFDrainResult> {
    const res = await apiClient.post("/waf/events/drain");
    return res.data.data;
  },
  async getLogs(params: {
    date?: string;
    trace_id?: string;
    search?: string;
    host?: string;
    client_ip?: string;
    rule_id?: string;
    route_type?: string;
    mode?: string;
    cursor?: string;
    limit?: string;
  }): Promise<WAFLogEntriesPayload> {
    const res = await apiClient.get("/waf/logs", { params });
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
      data: { date },
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
      tasks.push(
        apiClient
          .post("/ip-location/batch", { ips: batch })
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
  async getProvinces(): Promise<CidrProvincesPayload> {
    const res = await apiClient.get("/cidr/provinces");
    return res.data.data;
  },
  async getCities(province: string): Promise<CidrCitiesPayload> {
    const res = await apiClient.get("/cidr/cities", {
      params: { province },
    });
    return res.data.data;
  },
  async getSelector(province?: string): Promise<CidrSelectorPayload> {
    const res = await apiClient.get("/cidr/selector", {
      params: province ? { province } : undefined,
    });
    return res.data.data;
  },
  async getCidrs(payload: {
    province: string;
    city?: string | null;
  }): Promise<CidrLookupPayload> {
    const params: Record<string, string> = {
      province: payload.province,
    };
    if (payload.city) {
      params.city = payload.city;
    }
    const res = await apiClient.get("/cidr/cidrs", { params });
    return res.data.data;
  },
};
