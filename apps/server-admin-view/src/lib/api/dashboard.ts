import type {
  DashboardStats,
  HostActiveIpsPayload,
  StreamActiveIpsPayload,
  TrafficStats,
} from "../../types";
import type { operations as ApiContractOperations } from "@fn-knock/api-contract";
import { apiClient } from "./client";

type DashboardStatsOperation =
  ApiContractOperations["get_api_admin_dashboard_stats"];
type DashboardStatsQuery = NonNullable<
  DashboardStatsOperation["parameters"]["query"]
>;
type DashboardActiveIpsQuery =
  ApiContractOperations["get_api_admin_dashboard_active_ips"]["parameters"]["query"];
type DashboardStreamActiveIpsQuery =
  ApiContractOperations["get_api_admin_dashboard_stream_active_ips"]["parameters"]["query"];

export type {
  DashboardStats,
  HostActiveIpsPayload,
  StreamActiveIpsPayload,
  TrafficStats,
} from "../../types";

export const DashboardAPI = {
  async getStats(
    rangeSec: number,
    userIdOrOptions?:
      string | { userId?: string; host?: string; stream?: string },
    signal?: AbortSignal,
  ): Promise<DashboardStats> {
    const options =
      typeof userIdOrOptions === "string"
        ? { userId: userIdOrOptions }
        : (userIdOrOptions ?? {});
    const params = { rangeSec, ...options } satisfies DashboardStatsQuery;
    const res = await apiClient.get("/dashboard/stats", {
      params,
      signal,
    });
    return res.data.data;
  },
  async getRealtime(signal?: AbortSignal): Promise<TrafficStats> {
    const res = await apiClient.get("/dashboard/realtime", { signal });
    return res.data.data;
  },
  async getHostActiveIps(host: string): Promise<HostActiveIpsPayload> {
    const params = { host } satisfies DashboardActiveIpsQuery;
    const res = await apiClient.get("/dashboard/active-ips", {
      params,
    });
    return res.data.data;
  },
  async getStreamActiveIps(stream: string): Promise<StreamActiveIpsPayload> {
    const params = { stream } satisfies DashboardStreamActiveIpsQuery;
    const res = await apiClient.get("/dashboard/stream-active-ips", {
      params,
    });
    return res.data.data;
  },
};
