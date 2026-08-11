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

export const RuntimeHealthAPI = {
  async getHealth(): Promise<RuntimeHealthResponse> {
    const response = await apiClient.get("/runtime-health");
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
