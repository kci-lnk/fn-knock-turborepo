import type { RuntimeDebugResponse } from "../../types/runtime-debug";
import type { RuntimeLogComponent } from "../../types";
import type { operations as ApiContractOperations } from "@fn-knock/api-contract";
import { apiClient } from "./client";

type RuntimeHealthResponse =
  ApiContractOperations["get_api_admin_runtime_health"]["responses"][200]["content"]["application/json"];
type RuntimeDiagnosticsResponse =
  ApiContractOperations["get_api_admin_runtime_health_diagnostics"]["responses"][200]["content"]["application/json"];
type RuntimeLogsOperation =
  ApiContractOperations["get_api_admin_runtime_health_logs__component_"];
type RuntimeLogsResponse =
  RuntimeLogsOperation["responses"][200]["content"]["application/json"];
type RuntimeLogsQuery = NonNullable<
  RuntimeLogsOperation["parameters"]["query"]
>;
type RuntimeLogClearResponse =
  ApiContractOperations["delete_api_admin_runtime_health_logs__component_"]["responses"][200]["content"]["application/json"];
type GatewayMemoryConfigOperation =
  ApiContractOperations["put_api_admin_runtime_health_gateway_memory"];
type GatewayMemoryConfigResponse =
  ApiContractOperations["get_api_admin_runtime_health_gateway_memory"]["responses"][200]["content"]["application/json"];
type GatewayMemoryConfigUpdate =
  GatewayMemoryConfigOperation["requestBody"]["content"]["application/json"];
type GatewayMemoryReclaimResponse =
  ApiContractOperations["post_api_admin_runtime_health_gateway_memory_reclaim"]["responses"][200]["content"]["application/json"];

type RuntimeDebugCaptureResponse =
  ApiContractOperations["post_api_admin_runtime_health_debug_capture"]["responses"][200]["content"]["application/json"];
type RuntimeDebugStopResponse =
  ApiContractOperations["delete_api_admin_runtime_health_debug_capture"]["responses"][200]["content"]["application/json"];
type RuntimeDebugMemoryResponse =
  ApiContractOperations["post_api_admin_runtime_health_debug_memory"]["responses"][200]["content"]["application/json"];

export const RuntimeHealthAPI = {
  async getDebug(signal?: AbortSignal): Promise<RuntimeDebugResponse> {
    const response = await apiClient.get("/runtime-health/debug", { signal });
    return response.data;
  },

  async startDebugCapture(
    signal?: AbortSignal,
  ): Promise<RuntimeDebugCaptureResponse> {
    const response = await apiClient.post(
      "/runtime-health/debug/capture",
      {},
      { signal },
    );
    return response.data;
  },

  async stopDebugCapture(
    signal?: AbortSignal,
  ): Promise<RuntimeDebugStopResponse> {
    const response = await apiClient.delete("/runtime-health/debug/capture", {
      signal,
    });
    return response.data;
  },

  async refreshDebugMemory(
    signal?: AbortSignal,
  ): Promise<RuntimeDebugMemoryResponse> {
    const response = await apiClient.post(
      "/runtime-health/debug/memory",
      {},
      { signal },
    );
    return response.data;
  },

  async getHealth(signal?: AbortSignal): Promise<RuntimeHealthResponse> {
    const response = await apiClient.get("/runtime-health", { signal });
    return response.data;
  },

  async getDiagnostics(): Promise<RuntimeDiagnosticsResponse> {
    const response = await apiClient.get("/runtime-health/diagnostics");
    return response.data;
  },

  async getLogs(
    component: RuntimeLogComponent,
    limit = 200,
  ): Promise<RuntimeLogsResponse> {
    const params = { limit } satisfies RuntimeLogsQuery;
    const response = await apiClient.get(
      `/runtime-health/logs/${encodeURIComponent(component)}`,
      { params },
    );
    return response.data;
  },

  async clearLogs(
    component: RuntimeLogComponent,
  ): Promise<RuntimeLogClearResponse> {
    const response = await apiClient.delete(
      `/runtime-health/logs/${encodeURIComponent(component)}`,
    );
    return response.data;
  },

  async getGatewayMemoryConfig(): Promise<GatewayMemoryConfigResponse> {
    const response = await apiClient.get("/runtime-health/gateway-memory");
    return response.data;
  },

  async updateGatewayMemoryConfig(
    payload: GatewayMemoryConfigUpdate,
  ): Promise<GatewayMemoryConfigResponse> {
    const response = await apiClient.put(
      "/runtime-health/gateway-memory",
      payload,
    );
    return response.data;
  },

  async reclaimGatewayMemory(): Promise<GatewayMemoryReclaimResponse> {
    const response = await apiClient.post(
      "/runtime-health/gateway-memory/reclaim",
      {},
    );
    return response.data;
  },

  async downloadArchive(): Promise<{ blob: Blob; filename: string }> {
    const response = await apiClient.get(
      "/runtime-health/diagnostics/archive",
      { responseType: "blob" },
    );
    const disposition = String(response.headers["content-disposition"] || "");
    const filename =
      disposition.match(/filename="?([^";]+)"?/i)?.[1] ||
      `fn-knock-diagnostics-${Date.now()}.zip`;
    return { blob: response.data, filename };
  },
};
