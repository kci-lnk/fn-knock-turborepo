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

export interface WhitelistRegionInput {
  province: string;
  query_city?: string | null;
}

export interface WhitelistRegionGroupRecord {
  id: string;
  regions: WhitelistRegionInput[];
  cidrCount: number;
  expireAt: number | null;
  source: "manual";
  createdAt: number;
  updatedAt: number;
  status: "active" | "deleted" | "expired";
  comment?: string;
}

export interface WhitelistRegionAddResult {
  total: number;
  group: WhitelistRegionGroupRecord;
}

export const WhitelistAPI = {
  async getRecords() {
    const res = await apiClient.get("/whitelist");
    return res.data;
  },
  async getRegions(): Promise<{
    success: boolean;
    message?: string;
    data?: WhitelistRegionGroupRecord[];
  }> {
    const res = await apiClient.get("/whitelist/regions");
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
  async addRegions(payload: {
    regions: WhitelistRegionInput[];
    expireAt: number | null;
    comment?: string;
  }): Promise<{
    success: boolean;
    message?: string;
    data?: WhitelistRegionAddResult;
  }> {
    const res = await apiClient.post("/whitelist/regions", payload);
    return res.data;
  },
  async deleteRegion(id: string) {
    const res = await apiClient.delete(
      `/whitelist/regions/${encodeURIComponent(id)}`,
    );
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
