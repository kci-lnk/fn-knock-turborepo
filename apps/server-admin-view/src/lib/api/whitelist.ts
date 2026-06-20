import { apiClient } from "./client";

export interface WhiteListRecord {
  id: string;
  ip: string;
  targetType: "ip" | "cidr" | "cname";
  expireAt: number | null;
  source: "manual" | "auto";
  createdAt: number;
  comment?: string;
  status: "active" | "expired" | "deleted";
  ipLocation?: string;
  resolvedTargets?: string[];
  checkIntervalMinutes?: number | null;
  lastCheckedAt?: number | null;
  lastResolvedAt?: number | null;
  resolveStatus?: "pending" | "resolved" | "empty" | "error";
  resolveMessage?: string;
}

export const WhitelistAPI = {
  async getRecords() {
    const res = await apiClient.get("/whitelist");
    return res.data;
  },
  async addRecord(payload: {
    ip: string;
    targetType?: "ip" | "cidr" | "cname";
    expireAt: number | null;
    source: string;
    comment?: string;
    checkIntervalMinutes?: number;
  }) {
    const res = await apiClient.post("/whitelist", payload);
    return res.data;
  },
  async deleteRecord(id: string) {
    const res = await apiClient.delete(`/whitelist/${encodeURIComponent(id)}`);
    return res.data;
  },
  async updateComment(id: string, comment: string) {
    const res = await apiClient.patch(
      `/whitelist/${encodeURIComponent(id)}/comment`,
      { comment },
    );
    return res.data;
  },
  async refreshRecord(id: string): Promise<{
    success: boolean;
    message?: string;
    data?: {
      changed: boolean;
      skipped: boolean;
      record: WhiteListRecord;
    };
  }> {
    const res = await apiClient.post(
      `/whitelist/${encodeURIComponent(id)}/refresh`,
    );
    return res.data;
  },
};
