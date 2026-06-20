import type {
  DashboardStats,
  HostActiveIpsPayload,
  TrafficStats,
} from "../../types";
import { apiClient } from "./client";

export type {
  DashboardStats,
  HostActiveIpsPayload,
  TrafficStats,
} from "../../types";

export const DashboardAPI = {
  async getStats(
    rangeSec: number,
    userIdOrOptions?: string | { userId?: string; host?: string },
  ): Promise<DashboardStats> {
    const options =
      typeof userIdOrOptions === "string"
        ? { userId: userIdOrOptions }
        : (userIdOrOptions ?? {});
    const res = await apiClient.get("/dashboard/stats", {
      params: { rangeSec, ...options },
    });
    return res.data.data;
  },
  async getRealtime(): Promise<TrafficStats> {
    const res = await apiClient.get("/dashboard/realtime");
    return res.data.data;
  },
  async getHostActiveIps(host: string): Promise<HostActiveIpsPayload> {
    const res = await apiClient.get("/dashboard/active-ips", {
      params: { host },
    });
    return res.data.data;
  },
};
