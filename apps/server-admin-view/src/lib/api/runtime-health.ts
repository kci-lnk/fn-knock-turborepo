import type {
  RuntimeComponentLogs,
  RuntimeDiagnostics,
  RuntimeHealthSnapshot,
  RuntimeLogClearResult,
  RuntimeLogComponent,
} from "../../types";
import { apiClient } from "./client";

export const RuntimeHealthAPI = {
  async getHealth(): Promise<{
    success: boolean;
    data: RuntimeHealthSnapshot;
    message?: string;
  }> {
    const response = await apiClient.get("/runtime-health");
    return response.data;
  },

  async getDiagnostics(): Promise<{
    success: boolean;
    data: RuntimeDiagnostics;
    message?: string;
  }> {
    const response = await apiClient.get("/runtime-health/diagnostics");
    return response.data;
  },

  async getLogs(
    component: RuntimeLogComponent,
    limit = 200,
  ): Promise<{
    success: boolean;
    data: RuntimeComponentLogs;
    message?: string;
  }> {
    const response = await apiClient.get(
      `/runtime-health/logs/${encodeURIComponent(component)}`,
      { params: { limit } },
    );
    return response.data;
  },

  async clearLogs(component: RuntimeLogComponent): Promise<{
    success: boolean;
    data: RuntimeLogClearResult;
    message?: string;
  }> {
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
