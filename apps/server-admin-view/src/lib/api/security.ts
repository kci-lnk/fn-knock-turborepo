import type {
  SSHLoginLogListPayload,
  SSHSecurityBlockListPayload,
  SSHSecurityBlockRecord,
  SSHSecurityDetails,
  SSHSecurityFirewallClearResult,
  SSHSecurityFirewallSyncResult,
  ThreatOverview,
} from "../../types";
import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";
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

type SecuritySchemas = ApiContractComponents["schemas"];
type SshSecurityConfigUpdate = SecuritySchemas["SshSecurityConfigUpdateData"];
type SshBlocksDeleteBody = SecuritySchemas["SshBlocksDeleteBodyData"];
type SshLoginLogsQuery = NonNullable<
  ApiContractOperations["get_api_admin_ssh_security_login_logs"]["parameters"]["query"]
>;
type SshBlocksQuery = NonNullable<
  ApiContractOperations["get_api_admin_ssh_security_blocks"]["parameters"]["query"]
>;
type SshLoginLogsParams = {
  page: NonNullable<SshLoginLogsQuery["page"]>;
  limit: NonNullable<SshLoginLogsQuery["limit"]>;
  search?: SshLoginLogsQuery["search"];
  outcome?: SshLoginLogsQuery["outcome"] | "all";
};

export type ScannerSettings = SecuritySchemas["ScannerSettingsData"];
export type ScannerBlacklistHit = SecuritySchemas["ScannerBlacklistHitData"];
export type ScannerBlacklistRecord =
  SecuritySchemas["ScannerBlacklistRecordData"];
export type ScannerBlacklistList = SecuritySchemas["ScannerBlacklistListData"];
export type GeneralBlacklistSource =
  SecuritySchemas["GeneralBlacklistRecordData"]["source"];
export type GeneralBlacklistRecord =
  SecuritySchemas["GeneralBlacklistRecordData"];
export type GeneralBlacklistList = SecuritySchemas["GeneralBlacklistListData"];
export type GeneralBlacklistMutationResult =
  SecuritySchemas["GeneralBlacklistMutationData"];
export type GeneralBlacklistStatus =
  SecuritySchemas["GeneralBlacklistStatusData"];

type SecurityOverviewQuery = NonNullable<
  ApiContractOperations["get_api_admin_security_overview"]["parameters"]["query"]
>;
type ScannerSettingsUpdate = SecuritySchemas["ScannerSettingsUpdateData"];
type IpListBody = SecuritySchemas["IpListBodyData"];
type GeneralBlacklistAddBody = SecuritySchemas["GeneralBlacklistAddBodyData"];
type ScannerBlacklistQuery = NonNullable<
  ApiContractOperations["get_api_admin_scanner_blacklist"]["parameters"]["query"]
>;
type GeneralBlacklistQuery = NonNullable<
  ApiContractOperations["get_api_admin_general_blacklist"]["parameters"]["query"]
>;

export const SecurityAPI = {
  async getOverview(
    rangeSec: number,
    signal?: AbortSignal,
  ): Promise<ThreatOverview> {
    const params = { rangeSec } satisfies SecurityOverviewQuery;
    const res = await apiClient.get("/security/overview", {
      params,
      signal,
    });
    return res.data.data;
  },
};

export const ScannerAPI = {
  async getSettings(): Promise<ScannerSettings> {
    const res = await apiClient.get("/scanner/settings");
    return res.data.data;
  },
  async saveSettings(payload: ScannerSettingsUpdate): Promise<ScannerSettings> {
    const res = await apiClient.post("/scanner/settings", payload);
    return res.data.data;
  },
  async getBlacklist(
    page: number,
    limit: string,
    search: string,
  ): Promise<ScannerBlacklistList> {
    const params = { page, limit, search } satisfies ScannerBlacklistQuery;
    const res = await apiClient.get("/scanner/blacklist", {
      params,
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
    const body = { ips } satisfies IpListBody;
    await apiClient.delete("/scanner/blacklist", { data: body });
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
    const params = { page, limit, search } satisfies GeneralBlacklistQuery;
    const res = await apiClient.get("/general-blacklist", {
      params,
    });
    return res.data.data;
  },
  async add(
    ips: string[],
    source: GeneralBlacklistSource,
    comment?: string,
  ): Promise<GeneralBlacklistMutationResult> {
    const body = {
      ips,
      source,
      comment,
    } satisfies GeneralBlacklistAddBody;
    const res = await apiClient.post("/general-blacklist", body);
    return res.data.data;
  },
  async getStatus(ips: string[]): Promise<GeneralBlacklistStatus> {
    const body = { ips } satisfies IpListBody;
    const res = await apiClient.post("/general-blacklist/status", body);
    return res.data.data;
  },
  async delete(ips: string[]): Promise<GeneralBlacklistMutationResult> {
    const body = { ips } satisfies IpListBody;
    const res = await apiClient.delete("/general-blacklist", { data: body });
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
    payload: SshSecurityConfigUpdate,
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
  async getLoginLogs(
    params: SshLoginLogsParams,
  ): Promise<SSHLoginLogListPayload> {
    const query = {
      page: params.page,
      limit: params.limit,
      search: params.search || undefined,
      outcome:
        params.outcome && params.outcome !== "all" ? params.outcome : undefined,
    } satisfies SshLoginLogsQuery;
    const res = await apiClient.get("/ssh-security/login-logs", {
      params: query,
    });
    return res.data.data;
  },
  async getBlocks(
    page: number,
    limit: string,
    search: string,
  ): Promise<SSHSecurityBlockListPayload> {
    const params = { page, limit, search } satisfies SshBlocksQuery;
    const res = await apiClient.get("/ssh-security/blocks", {
      params,
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
    const body = { ips } satisfies SshBlocksDeleteBody;
    await apiClient.delete("/ssh-security/blocks", { data: body });
  },
};
