import type {
  SSHLoginLogListPayload,
  SSHSecurityBlockListPayload,
  SSHSecurityBlockRecord,
  SSHSecurityConfig,
  SSHSecurityDetails,
  SSHSecurityFirewallClearResult,
  SSHSecurityFirewallSyncResult,
  ThreatOverview,
} from "../../types";
import { apiClient } from "./client";

export type {
  SSHLoginLogListPayload,
  SSHSecurityBlockListPayload,
  SSHSecurityBlockRecord,
  SSHSecurityConfig,
  SSHSecurityDetails,
  SSHSecurityFirewallClearResult,
  SSHSecurityFirewallSyncResult,
  ThreatOverview,
} from "../../types";

export type ScannerSettings = {
  enabled: boolean;
  windowMinutes: number;
  threshold: number;
  windowSeconds: number;
  blacklistTtlSeconds: number;
  commonLocationExemptEnabled: boolean;
};

export type ScannerBlacklistHit = {
  path: string;
  createdAt: number;
};

export type ScannerBlacklistRecord = {
  ip: string;
  ipLocation?: string;
  blockedAt: number;
  windowMinutes: number;
  threshold: number;
  hits: ScannerBlacklistHit[];
};

export type ScannerBlacklistList = {
  items: ScannerBlacklistRecord[];
  total: number;
};

export type GeneralBlacklistSource =
  | "manual"
  | "request_log"
  | "active_ip"
  | "waf_log";

export type GeneralBlacklistRecord = {
  ip: string;
  source?: GeneralBlacklistSource | string;
  comment?: string;
  created_at?: string;
  updated_at?: string;
  ipLocation?: string;
};

export type GeneralBlacklistList = {
  items: GeneralBlacklistRecord[];
  total: number;
};

export type GeneralBlacklistMutationResult = {
  added: number;
  updated: number;
  removed: number;
  total: number;
  items: GeneralBlacklistRecord[];
};

export type GeneralBlacklistStatus = {
  records: Record<string, GeneralBlacklistRecord>;
};

export const SecurityAPI = {
  async getOverview(rangeSec: number): Promise<ThreatOverview> {
    const res = await apiClient.get("/security/overview", {
      params: { rangeSec },
    });
    return res.data.data;
  },
};

export const ScannerAPI = {
  async getSettings(): Promise<ScannerSettings> {
    const res = await apiClient.get("/scanner/settings");
    return res.data.data;
  },
  async saveSettings(payload: {
    enabled: boolean;
    windowMinutes: number;
    threshold: number;
    blacklistTtlSeconds: number;
    commonLocationExemptEnabled?: boolean;
  }): Promise<ScannerSettings> {
    const res = await apiClient.post("/scanner/settings", payload);
    return res.data.data;
  },
  async getBlacklist(
    page: number,
    limit: string,
    search: string,
  ): Promise<ScannerBlacklistList> {
    const res = await apiClient.get("/scanner/blacklist", {
      params: { page, limit, search },
    });
    return res.data.data;
  },
  async getBlacklistDetail(ip: string): Promise<ScannerBlacklistRecord> {
    const res = await apiClient.get(
      `/scanner/blacklist/${encodeURIComponent(ip)}`,
    );
    return res.data.data;
  },
  async deleteBlacklist(ips: string[]): Promise<void> {
    await apiClient.delete("/scanner/blacklist", { data: { ips } });
  },
  async deleteBlacklistByIp(ip: string): Promise<void> {
    await apiClient.delete(`/scanner/blacklist/${encodeURIComponent(ip)}`);
  },
};

export const GeneralBlacklistAPI = {
  async getList(
    page: number,
    limit: string,
    search: string,
  ): Promise<GeneralBlacklistList> {
    const res = await apiClient.get("/general-blacklist", {
      params: { page, limit, search },
    });
    return res.data.data;
  },
  async add(
    ips: string[],
    source: GeneralBlacklistSource,
    comment?: string,
  ): Promise<GeneralBlacklistMutationResult> {
    const res = await apiClient.post("/general-blacklist", {
      ips,
      source,
      comment,
    });
    return res.data.data;
  },
  async getStatus(ips: string[]): Promise<GeneralBlacklistStatus> {
    const res = await apiClient.post("/general-blacklist/status", { ips });
    return res.data.data;
  },
  async delete(ips: string[]): Promise<GeneralBlacklistMutationResult> {
    const res = await apiClient.delete("/general-blacklist", { data: { ips } });
    return res.data.data;
  },
  async deleteByIp(ip: string): Promise<GeneralBlacklistMutationResult> {
    const res = await apiClient.delete(
      `/general-blacklist/${encodeURIComponent(ip)}`,
    );
    return res.data.data;
  },
};

export const SSHSecurityAPI = {
  async getDetails(): Promise<SSHSecurityDetails> {
    const res = await apiClient.get("/ssh-security/config");
    return res.data.data;
  },
  async updateConfig(
    payload: Partial<Omit<SSHSecurityConfig, "allowed_regions">> & {
      allowed_regions?: Array<{
        province: string;
        query_city?: string | null;
      }>;
    },
  ): Promise<SSHSecurityDetails> {
    const res = await apiClient.post("/ssh-security/config", payload);
    return res.data.data;
  },
  async syncFirewall(): Promise<SSHSecurityFirewallSyncResult> {
    const res = await apiClient.post("/ssh-security/firewall/sync");
    return res.data.data;
  },
  async clearFirewall(): Promise<SSHSecurityFirewallClearResult> {
    const res = await apiClient.post("/ssh-security/firewall/clear");
    return res.data.data;
  },
  async getLoginLogs(params: {
    page: number;
    limit: string;
    search?: string;
    outcome?: "success" | "failure" | "all";
  }): Promise<SSHLoginLogListPayload> {
    const res = await apiClient.get("/ssh-security/login-logs", {
      params: {
        page: params.page,
        limit: params.limit,
        search: params.search || undefined,
        outcome:
          params.outcome && params.outcome !== "all"
            ? params.outcome
            : undefined,
      },
    });
    return res.data.data;
  },
  async getBlocks(
    page: number,
    limit: string,
    search: string,
  ): Promise<SSHSecurityBlockListPayload> {
    const res = await apiClient.get("/ssh-security/blocks", {
      params: { page, limit, search },
    });
    return res.data.data;
  },
  async getBlock(ip: string): Promise<SSHSecurityBlockRecord> {
    const res = await apiClient.get(
      `/ssh-security/blocks/${encodeURIComponent(ip)}`,
    );
    return res.data.data;
  },
  async deleteBlock(ip: string): Promise<void> {
    await apiClient.delete(`/ssh-security/blocks/${encodeURIComponent(ip)}`);
  },
  async deleteBlocks(ips: string[]): Promise<void> {
    await apiClient.delete("/ssh-security/blocks", { data: { ips } });
  },
};
