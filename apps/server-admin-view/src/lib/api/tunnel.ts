import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";
import { apiClient } from "./client";

type TunnelSchemas = ApiContractComponents["schemas"];

export type FrpcTcpItem = TunnelSchemas["FrpcTcpItemData"];

export type FrpcInstanceSummary =
  TunnelSchemas["FrpcInstanceSummaryData"];

export type TunnelSupervisorFailure =
  TunnelSchemas["CloudflaredSupervisorFailureData"];

export type TunnelSupervisorStatus = TunnelSchemas["CloudflaredSupervisorData"];

export type FrpcInstanceStatus =
  TunnelSchemas["FrpcInstanceStatusData"];

export type FrpcInstancesOverview =
  TunnelSchemas["FrpcInstancesOverviewData"];

export type FrpcInstanceDetail =
  TunnelSchemas["FrpcInstanceDetailData"];

export type FrpcStatusPayload = TunnelSchemas["FrpcPrimaryStatusData"];

export type FrpcPollPayload = TunnelSchemas["FrpcPollData"];

export type FrpcInstancePollPayload =
  TunnelSchemas["FrpcInstancePollData"];

export type CloudflaredProtocol =
  TunnelSchemas["CloudflaredConfigData"]["protocol"];
export type CloudflaredConfig = TunnelSchemas["CloudflaredConfigData"];
export type CloudflareTunnelSummary =
  TunnelSchemas["CloudflareTunnelSummaryData"];
export type CloudflareOptimizationCandidate =
  TunnelSchemas["CloudflareOptimizationCandidateData"];
export type CloudflareOptimizationVantage =
  TunnelSchemas["CloudflareOptimizationVantageData"];
export type CloudflareOptimizationCandidateSources =
  TunnelSchemas["CloudflareOptimizationCandidateSourcesData"];
export type CloudflareOptimizationScan =
  TunnelSchemas["CloudflareOptimizationScanData"];
export type CloudflareOptimizationDomain =
  TunnelSchemas["CloudflareOptimizationDomainData"];
export type CloudflareManagedState =
  TunnelSchemas["CloudflareManagedStateData"];
export type CloudflareReconcileOperation =
  TunnelSchemas["CloudflareReconcileOperationData"];
export type CloudflareReconcileConflict =
  TunnelSchemas["CloudflareReconcileConflictData"];
export type CloudflareReconcilePlan =
  TunnelSchemas["CloudflareReconcilePlanData"];
export type CloudflaredStatusPayload =
  TunnelSchemas["CloudflaredRuntimeStatusData"];
export type CloudflaredPollPayload = TunnelSchemas["CloudflaredPollData"];

type CloudflaredStatus = TunnelSchemas["CloudflaredStatusData"];
type CloudflaredConfigUpdate = TunnelSchemas["CloudflaredConfigUpdateData"];
type CloudflareCredentialBody = TunnelSchemas["CloudflareCredentialBodyData"];
type CloudflareReconcileRequest =
  TunnelSchemas["CloudflareReconcileRequestData"];
type CloudflareReconcileApplyBody =
  TunnelSchemas["CloudflareReconcileApplyBodyData"];
type CloudflareOptimizationSourceSettingsBody =
  TunnelSchemas["CloudflareOptimizationSourceSettingsBodyData"];
type CloudflareOptimizationDomainBody =
  TunnelSchemas["CloudflareOptimizationDomainBodyData"];
type CloudflareOptimizationDomainUpdate =
  TunnelSchemas["CloudflareOptimizationDomainUpdateData"];
type CloudflareOptimizationApplyBody =
  TunnelSchemas["CloudflareOptimizationApplyBodyData"];
type CloudflareOptimizationApply =
  TunnelSchemas["CloudflareOptimizationApplyData"];
type CloudflareOptimizationFallback =
  TunnelSchemas["CloudflareOptimizationFallbackData"];
type CloudflaredLogsQuery = NonNullable<
  ApiContractOperations["get_api_admin_cloudflared_logs"]["parameters"]["query"]
>;
type CloudflaredPollQuery = NonNullable<
  ApiContractOperations["get_api_admin_cloudflared_poll"]["parameters"]["query"]
>;
type FrpcStatus = TunnelSchemas["FrpcStatusData"];
type FrpcLegacyOverview = TunnelSchemas["FrpcLegacyOverviewData"];
type FrpcWebStatus = TunnelSchemas["FrpcWebStatusData"];
type FrpcConfig = TunnelSchemas["FrpcConfigData"];
type FrpcConfigUpdate = TunnelSchemas["FrpcConfigUpdateData"];
type FrpcStart = TunnelSchemas["FrpcStartData"];
type FrpcInstanceBody = TunnelSchemas["FrpcInstanceBodyData"];
type FrpcOverviewQuery = NonNullable<
  ApiContractOperations["get_api_admin_frpc_overview"]["parameters"]["query"]
>;
type FrpcLogsQuery = NonNullable<
  ApiContractOperations["get_api_admin_frpc_logs"]["parameters"]["query"]
>;
type FrpcPollQuery = NonNullable<
  ApiContractOperations["get_api_admin_frpc_poll"]["parameters"]["query"]
>;
type FrpcInstanceQuery = NonNullable<
  ApiContractOperations["get_api_admin_frpc_instances__id_"]["parameters"]["query"]
>;
type FrpcInstanceLogsQuery = NonNullable<
  ApiContractOperations["get_api_admin_frpc_instances__id__logs"]["parameters"]["query"]
>;
type FrpcInstancePollQuery = NonNullable<
  ApiContractOperations["get_api_admin_frpc_instances__id__poll"]["parameters"]["query"]
>;

export const FrpcAPI = {
  async getStatus(): Promise<FrpcStatus> {
    const res = await apiClient.get("/frpc/status");
    return res.data.data;
  },
  async getOverview(limit = 200): Promise<FrpcLegacyOverview> {
    const params = { limit } satisfies FrpcOverviewQuery;
    const res = await apiClient.get("/frpc/overview", { params });
    return res.data.data;
  },
  async getWebStatus(): Promise<FrpcWebStatus> {
    const res = await apiClient.get("/frpc/web-status");
    return res.data.data;
  },
  async getConfig(): Promise<string> {
    const res = await apiClient.get("/frpc/config");
    const data: FrpcConfig = res.data.data;
    return data.content;
  },
  async saveConfig(content: string): Promise<void> {
    const payload = { content } satisfies FrpcConfigUpdate;
    await apiClient.post("/frpc/config", payload);
  },
  async start(): Promise<FrpcStart> {
    const res = await apiClient.post("/frpc/start");
    return res.data.data;
  },
  async stop(): Promise<void> {
    await apiClient.post("/frpc/stop");
  },
  async getLogs(limit = 200): Promise<string[]> {
    const params = { limit } satisfies FrpcLogsQuery;
    const res = await apiClient.get("/frpc/logs", { params });
    return res.data.data;
  },
  async clearLogs(): Promise<void> {
    await apiClient.delete("/frpc/logs");
  },
  async poll(cursor?: number): Promise<FrpcPollPayload> {
    const params =
      typeof cursor === "number"
        ? ({ cursor } satisfies FrpcPollQuery)
        : undefined;
    const res = await apiClient.get("/frpc/poll", {
      params,
    });
    return res.data.data;
  },
  async getInstances(): Promise<FrpcInstancesOverview> {
    const res = await apiClient.get("/frpc/instances");
    return res.data.data;
  },
  async createDraft(): Promise<string> {
    const res = await apiClient.post("/frpc/instances/draft");
    const data: FrpcConfig = res.data.data;
    return data.content;
  },
  async createInstance(
    payload: FrpcInstanceBody,
  ): Promise<FrpcInstanceStatus> {
    const res = await apiClient.post("/frpc/instances", payload);
    return res.data.data;
  },
  async getInstance(id: string, limit = 200): Promise<FrpcInstanceDetail> {
    const params = { limit } satisfies FrpcInstanceQuery;
    const res = await apiClient.get(
      `/frpc/instances/${encodeURIComponent(id)}`,
      { params },
    );
    return res.data.data;
  },
  async updateInstance(
    id: string,
    payload: FrpcInstanceBody,
  ): Promise<FrpcInstanceStatus> {
    const res = await apiClient.put(
      `/frpc/instances/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data.data;
  },
  async deleteInstance(id: string): Promise<void> {
    await apiClient.delete(`/frpc/instances/${encodeURIComponent(id)}`);
  },
  async startInstance(id: string): Promise<FrpcStart> {
    const res = await apiClient.post(
      `/frpc/instances/${encodeURIComponent(id)}/start`,
    );
    return res.data.data;
  },
  async stopInstance(id: string): Promise<void> {
    await apiClient.post(`/frpc/instances/${encodeURIComponent(id)}/stop`);
  },
  async restartInstance(id: string): Promise<FrpcStart> {
    const res = await apiClient.post(
      `/frpc/instances/${encodeURIComponent(id)}/restart`,
    );
    return res.data.data;
  },
  async getInstanceLogs(id: string, limit = 200): Promise<string[]> {
    const params = { limit } satisfies FrpcInstanceLogsQuery;
    const res = await apiClient.get(
      `/frpc/instances/${encodeURIComponent(id)}/logs`,
      { params },
    );
    return res.data.data;
  },
  async clearInstanceLogs(id: string): Promise<void> {
    await apiClient.delete(`/frpc/instances/${encodeURIComponent(id)}/logs`);
  },
  async pollInstance(
    id: string,
    cursor?: number,
  ): Promise<FrpcInstancePollPayload> {
    const params =
      typeof cursor === "number"
        ? ({ cursor } satisfies FrpcInstancePollQuery)
        : undefined;
    const res = await apiClient.get(
      `/frpc/instances/${encodeURIComponent(id)}/poll`,
      { params },
    );
    return res.data.data;
  },
};

export const CloudflaredAPI = {
  async getStatus(): Promise<CloudflaredStatus> {
    const res = await apiClient.get("/cloudflared/status");
    return res.data.data;
  },
  async getConfig(): Promise<CloudflaredConfig> {
    const res = await apiClient.get("/cloudflared/config");
    return res.data.data;
  },
  async saveConfig(config: CloudflaredConfigUpdate): Promise<void> {
    await apiClient.post("/cloudflared/config", config);
  },
  async saveCloudflareCredential(
    apiToken: string,
  ): Promise<CloudflareManagedState> {
    const body = { apiToken } satisfies CloudflareCredentialBody;
    const res = await apiClient.put("/cloudflared/cloudflare/credential", body);
    return res.data.data;
  },
  async deleteCloudflareCredential(): Promise<void> {
    await apiClient.delete("/cloudflared/cloudflare/credential");
  },
  async getCloudflareState(): Promise<CloudflareManagedState> {
    const res = await apiClient.get("/cloudflared/cloudflare/state");
    return res.data.data;
  },
  async previewReconcile(
    payload: CloudflareReconcileRequest,
  ): Promise<CloudflareReconcilePlan> {
    const res = await apiClient.post("/cloudflared/reconcile/preview", payload);
    return res.data.data;
  },
  async applyReconcile(
    payload: CloudflareReconcileApplyBody,
  ): Promise<CloudflareManagedState> {
    const res = await apiClient.post("/cloudflared/reconcile/apply", payload);
    return res.data.data;
  },
  async startOptimizationScan(): Promise<CloudflareOptimizationScan> {
    const res = await apiClient.post("/cloudflared/optimization/scans");
    return res.data.data;
  },
  async saveOptimizationSourceSettings(
    payload: CloudflareOptimizationSourceSettingsBody,
  ): Promise<CloudflareOptimizationCandidateSources> {
    const res = await apiClient.put(
      "/cloudflared/optimization/settings",
      payload,
    );
    return res.data.data;
  },
  async setOptimizationDomainMode(
    hostname: string,
    mode: CloudflareOptimizationDomainBody["mode"],
  ): Promise<CloudflareOptimizationDomainUpdate> {
    const body = { mode } satisfies CloudflareOptimizationDomainBody;
    const res = await apiClient.put(
      `/cloudflared/optimization/domains/${encodeURIComponent(hostname)}`,
      body,
    );
    return res.data.data;
  },
  async getOptimizationScan(id: string): Promise<CloudflareOptimizationScan> {
    const res = await apiClient.get(
      `/cloudflared/optimization/scans/${encodeURIComponent(id)}`,
    );
    return res.data.data;
  },
  async cancelOptimizationScan(id: string): Promise<void> {
    await apiClient.delete(
      `/cloudflared/optimization/scans/${encodeURIComponent(id)}`,
    );
  },
  async applyOptimization(
    payload: CloudflareOptimizationApplyBody,
  ): Promise<CloudflareOptimizationApply> {
    const res = await apiClient.post(
      "/cloudflared/optimization/apply",
      payload,
    );
    return res.data.data;
  },
  async fallbackOptimization(): Promise<CloudflareOptimizationFallback> {
    const res = await apiClient.post("/cloudflared/optimization/fallback");
    return res.data.data;
  },
  async start(): Promise<{ pid: number }> {
    const res = await apiClient.post("/cloudflared/start");
    return res.data.data;
  },
  async stop(): Promise<void> {
    await apiClient.post("/cloudflared/stop");
  },
  async getLogs(limit = 200): Promise<string[]> {
    const params = { limit } satisfies CloudflaredLogsQuery;
    const res = await apiClient.get("/cloudflared/logs", { params });
    return res.data.data;
  },
  async clearLogs(): Promise<void> {
    await apiClient.delete("/cloudflared/logs");
  },
  async poll(cursor?: number): Promise<CloudflaredPollPayload> {
    const params = (
      typeof cursor === "number" ? { cursor } : undefined
    ) satisfies CloudflaredPollQuery | undefined;
    const res = await apiClient.get("/cloudflared/poll", {
      params,
    });
    return res.data.data;
  },
};
