import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";
import { apiClient } from "./client";

type DdnsSchemas = ApiContractComponents["schemas"];

export type DDNSLogEntry = DdnsSchemas["DdnsLogEntryData"];
export type DDNSStatusPayload = DdnsSchemas["DdnsStatusData"];
export type DDNSSettingsPayload = DdnsSchemas["DdnsSettingsData"];
export type DDNSSettingsUpdatePayload = DdnsSchemas["DdnsSettingsUpdateData"];
export type DDNSTargetSummaryPayload = DdnsSchemas["DdnsTargetSummaryData"];
export type DDNSTargetDetailPayload = DdnsSchemas["DdnsTargetDetailData"];
export type DDNSTargetListPayload = DdnsSchemas["DdnsTargetListData"];
export type DDNSNetworkInterfaceAddress =
  DdnsSchemas["DdnsNetworkInterfaceAddressData"];
export type DDNSNetworkInterfacePayload =
  DdnsSchemas["DdnsNetworkInterfaceData"];
export type DDNSInterfaceSelector = DdnsSchemas["DdnsInterfaceSelectorData"];
export type DDNSInterfaceSelectorPreviewPayload =
  DdnsSchemas["DdnsInterfaceSelectorPreviewData"];
export type DDNSPollPayload = DdnsSchemas["DdnsPollData"];
export type DDNSProviderCapabilities =
  DdnsSchemas["DdnsProviderCapabilitiesData"];
export type DDNSPublicCheckSourcesPayload =
  DdnsSchemas["DdnsPublicCheckSourcesData"];
export type DDNSPublicCheckTestResultPayload =
  DdnsSchemas["DdnsPublicCheckTestResultData"];
export type DDNSIpSource = DDNSStatusPayload["ipSource"];
export type DDNSUpdateScope = DDNSStatusPayload["updateScope"];
export type DDNSHttpTransport = DDNSSettingsPayload["httpTransport"];
export type DDNSPublicDnsProvider = DDNSSettingsPayload["publicDnsProvider"];
export type DDNSPublicCheckFamily = DDNSPublicCheckTestResultPayload["family"];

type DdnsPublicCheckTestBody = DdnsSchemas["DdnsPublicCheckTestBodyData"];
type DdnsPublicCheckTestResults = DdnsSchemas["DdnsPublicCheckTestResultsData"];
type DdnsProvider = DdnsSchemas["DdnsProviderData"];
type DdnsInterfaceSelectorPreviewBody =
  DdnsSchemas["DdnsInterfaceSelectorPreviewBodyData"];
type DdnsProviderBody = DdnsSchemas["DdnsProviderBodyData"];
type DdnsConfig = DdnsSchemas["DdnsConfigData"];
type DdnsConfigBody = DdnsSchemas["DdnsConfigBodyData"];
type DdnsTargetBody = DdnsSchemas["DdnsTargetBodyData"];
type DdnsTargetEnabledBody = DdnsSchemas["DdnsTargetEnabledBodyData"];
type DdnsTestResponse = DdnsSchemas["DdnsTestResponseData"];
type DdnsLogsQuery = NonNullable<
  ApiContractOperations["get_api_admin_ddns_logs"]["parameters"]["query"]
>;
type DdnsPollQuery = NonNullable<
  ApiContractOperations["get_api_admin_ddns_poll"]["parameters"]["query"]
>;

export const DDNSAPI = {
  async getStatus(): Promise<DDNSStatusPayload> {
    const res = await apiClient.get("/ddns/status");
    return res.data.data;
  },
  async toggle(enabled: boolean): Promise<void> {
    const payload = { enabled } satisfies DdnsSchemas["DdnsToggleBodyData"];
    await apiClient.post("/ddns/toggle", payload);
  },
  async getSettings(): Promise<DDNSSettingsPayload> {
    const res = await apiClient.get("/ddns/settings");
    return res.data.data;
  },
  async saveSettings(
    payload: DDNSSettingsUpdatePayload,
  ): Promise<DDNSSettingsPayload> {
    const res = await apiClient.post("/ddns/settings", payload);
    return res.data.data;
  },
  async testPublicCheckSources(
    publicCheckSources: DDNSPublicCheckSourcesPayload,
    options: {
      httpTransport?: DDNSHttpTransport;
      publicDnsProvider?: DDNSPublicDnsProvider;
      networkInterface?: string;
    } = {},
  ): Promise<DdnsPublicCheckTestResults> {
    const payload = {
      publicCheckSources,
      ...options,
    } satisfies DdnsPublicCheckTestBody;
    const res = await apiClient.post("/ddns/public-check/test", payload);
    return res.data.data;
  },
  async getProviders(): Promise<DdnsProvider[]> {
    const res = await apiClient.get("/ddns/providers");
    return res.data.data;
  },
  async getNetworkInterfaces(): Promise<DDNSNetworkInterfacePayload[]> {
    const res = await apiClient.get("/ddns/interfaces");
    return res.data.data;
  },
  async resolveInterfaceSelector(
    payload: DdnsInterfaceSelectorPreviewBody,
    signal?: AbortSignal,
  ): Promise<DDNSInterfaceSelectorPreviewPayload> {
    const res = await apiClient.post("/ddns/interfaces/resolve", payload, {
      signal,
    });
    return res.data.data;
  },
  async setProvider(provider: string): Promise<void> {
    const payload = {
      provider: provider as DdnsProviderBody["provider"],
    } satisfies DdnsProviderBody;
    await apiClient.post("/ddns/provider", payload);
  },
  async getConfig(provider: string): Promise<Record<string, string>> {
    const res = await apiClient.get(
      `/ddns/config/${encodeURIComponent(provider)}`,
    );
    const data: DdnsConfig = res.data.data;
    return data;
  },
  async saveConfig(
    provider: string,
    config: Record<string, string>,
  ): Promise<void> {
    const payload = {
      config,
    } satisfies DdnsConfigBody;
    await apiClient.post(
      `/ddns/config/${encodeURIComponent(provider)}`,
      payload,
    );
  },
  async test(): Promise<DdnsTestResponse> {
    const res = await apiClient.post("/ddns/test");
    return res.data;
  },
  async getTargets(): Promise<DDNSTargetListPayload> {
    const res = await apiClient.get("/ddns/targets");
    return res.data.data;
  },
  async getTarget(id: string): Promise<DDNSTargetDetailPayload> {
    const res = await apiClient.get(`/ddns/targets/${encodeURIComponent(id)}`);
    return res.data.data;
  },
  async createTarget(payload: {
    name?: string;
    provider: string;
    enabled?: boolean;
    config: Record<string, string>;
  }): Promise<DDNSTargetDetailPayload> {
    const body = {
      ...payload,
      provider: payload.provider as DdnsTargetBody["provider"],
    } satisfies DdnsTargetBody;
    const res = await apiClient.post("/ddns/targets", body);
    return res.data.data;
  },
  async updateTarget(
    id: string,
    payload: {
      name?: string;
      provider: string;
      enabled?: boolean;
      config: Record<string, string>;
    },
  ): Promise<DDNSTargetDetailPayload> {
    const body = {
      ...payload,
      provider: payload.provider as DdnsTargetBody["provider"],
    } satisfies DdnsTargetBody;
    const res = await apiClient.put(
      `/ddns/targets/${encodeURIComponent(id)}`,
      body,
    );
    return res.data.data;
  },
  async deleteTarget(id: string): Promise<void> {
    await apiClient.delete(`/ddns/targets/${encodeURIComponent(id)}`);
  },
  async setTargetEnabled(id: string, enabled: boolean): Promise<void> {
    const payload = {
      enabled,
    } satisfies DdnsTargetEnabledBody;
    await apiClient.post(
      `/ddns/targets/${encodeURIComponent(id)}/enabled`,
      payload,
    );
  },
  async testTarget(id: string): Promise<DdnsTestResponse> {
    const res = await apiClient.post(
      `/ddns/targets/${encodeURIComponent(id)}/test`,
    );
    return res.data;
  },
  async getLogs(limit = 200): Promise<DDNSLogEntry[]> {
    const params = { limit } satisfies DdnsLogsQuery;
    const res = await apiClient.get("/ddns/logs", { params });
    return res.data.data;
  },
  async clearLogs(): Promise<void> {
    await apiClient.delete("/ddns/logs");
  },
  async poll(cursor?: number, signal?: AbortSignal): Promise<DDNSPollPayload> {
    const params = (
      typeof cursor === "number" ? { cursor } : undefined
    ) satisfies DdnsPollQuery | undefined;
    const res = await apiClient.get("/ddns/poll", {
      params,
      signal,
    });
    return res.data.data;
  },
};
