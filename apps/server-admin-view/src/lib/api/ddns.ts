import { apiClient } from "./client";
import type { DDNSDomainTargetsCapability } from "../ddns-domain";

export type DDNSLogEntry = {
  time: string;
  level: "info" | "error" | "warn";
  message: string;
};

export type DDNSIpSource = "public" | "interface" | "static" | "domain";
export type DDNSUpdateScope = "dual_stack" | "ipv6_only" | "ipv4_only";
export type DDNSHttpTransport = "curl" | "node";
export type DDNSPublicDnsProvider =
  | "none"
  | "alidns"
  | "tencent"
  | "cloudflare"
  | "google";

export type DDNSProviderCapabilities = {
  addressMode?: "dual_stack" | "single_address";
  ipSources?: DDNSIpSource[];
  domainTargets?: DDNSDomainTargetsCapability;
};

export type DDNSPublicCheckFamily = "ipv4" | "ipv6";

export type DDNSPublicCheckSourcesPayload = Record<
  DDNSPublicCheckFamily,
  string[]
>;

export type DDNSPublicCheckTestResultPayload = {
  family: DDNSPublicCheckFamily;
  url: string;
  success: boolean;
  status: number | null;
  ip: string | null;
  responsePreview?: string;
  error?: string;
};

export type DDNSStatusPayload = {
  enabled: boolean;
  provider: string | null;
  updateIntervalMinutes: number;
  publicCheckSources: DDNSPublicCheckSourcesPayload;
  defaultPublicCheckSources: DDNSPublicCheckSourcesPayload;
  httpTransport: DDNSHttpTransport;
  publicDnsProvider: DDNSPublicDnsProvider;
  updateScope: DDNSUpdateScope;
  ipSource: DDNSIpSource;
  networkInterface: string;
  lastIP: {
    ipv4: string | null;
    ipv6: string | null;
    updated_at: string | null;
  };
  selectionAnchor: {
    ipv4: string | null;
    ipv6: string | null;
    updated_at: string | null;
  };
  lastCheck: {
    checked_at: string | null;
    outcome: "updated" | "noop" | "skipped" | "error" | null;
    message: string | null;
  };
  primaryTargetId: string | null;
  extraTargetCount: number;
  enabledExtraTargetCount: number;
  targets: DDNSTargetSummaryPayload[];
};

export type DDNSSettingsPayload = {
  updateIntervalMinutes: number;
  publicCheckSources: DDNSPublicCheckSourcesPayload;
  defaultPublicCheckSources: DDNSPublicCheckSourcesPayload;
  httpTransport: DDNSHttpTransport;
  publicDnsProvider: DDNSPublicDnsProvider;
};

export type DDNSSettingsUpdatePayload = Partial<
  Pick<
    DDNSSettingsPayload,
    | "updateIntervalMinutes"
    | "publicCheckSources"
    | "httpTransport"
    | "publicDnsProvider"
  >
>;

export type DDNSTargetSummaryPayload = {
  id: string;
  name: string;
  isPrimary: boolean;
  enabled: boolean;
  provider: string | null;
  updateScope: DDNSUpdateScope;
  providerLabel: string;
  domainSummary: string;
  createdAt: string;
  updatedAt: string;
  sortOrder: number;
  lastIP: {
    ipv4: string | null;
    ipv6: string | null;
    updated_at: string | null;
  };
  selectionAnchor: {
    ipv4: string | null;
    ipv6: string | null;
    updated_at: string | null;
  };
  lastCheck: {
    checked_at: string | null;
    outcome: "updated" | "noop" | "skipped" | "error" | null;
    message: string | null;
  };
};

export type DDNSTargetDetailPayload = DDNSTargetSummaryPayload & {
  rawName?: string;
  config: Record<string, string>;
};

export type DDNSTargetListPayload = {
  primaryTargetId: string | null;
  total: number;
  extraCount: number;
  enabledExtraCount: number;
  items: DDNSTargetSummaryPayload[];
};

export type DDNSNetworkInterfacePayload = {
  name: string;
  label: string;
  summary: string;
  hasIpv4: boolean;
  hasIpv6: boolean;
  addresses: Array<{
    family: "ipv4" | "ipv6";
    address: string;
    cidr: string | null;
    prefixLength?: number | null;
    internal: boolean;
    source?: "runtime" | "docker_host";
    temporary?: boolean | null;
    deprecated?: boolean | null;
    tentative?: boolean | null;
    dadFailed?: boolean | null;
  }>;
  selectableAddresses: Array<{
    family: "ipv4" | "ipv6";
    address: string;
    cidr: string | null;
    prefixLength?: number | null;
    internal: boolean;
    source?: "runtime" | "docker_host";
    temporary?: boolean | null;
    deprecated?: boolean | null;
    tentative?: boolean | null;
    dadFailed?: boolean | null;
  }>;
  source?: "runtime" | "docker_host";
};

export type DDNSInterfaceSelector = {
  version: 1;
  mode: "auto" | "rules";
  preferredAddress?: string;
  includeCidrs?: string[];
  excludeCidrs?: string[];
  ipv6InterfaceId?: string;
  allowTemporary: boolean;
};

export type DDNSInterfaceSelectorPreviewPayload = {
  selectedAddress: string | null;
  matchedAddresses: DDNSNetworkInterfacePayload["selectableAddresses"];
  rejectedAddresses: Array<{ address: string | null; reasons: string[] }>;
  reason: "current" | "preferred" | "ranked" | "no_match";
  warnings: Array<"multiple_matches" | "status_unknown">;
  selector: DDNSInterfaceSelector;
};

export type DDNSPollPayload = {
  cursor: number;
  reset: boolean;
  logs: DDNSLogEntry[];
  status: DDNSStatusPayload;
};

export const DDNSAPI = {
  async getStatus(): Promise<DDNSStatusPayload> {
    const res = await apiClient.get("/ddns/status");
    return res.data.data;
  },
  async toggle(enabled: boolean): Promise<void> {
    await apiClient.post("/ddns/toggle", { enabled });
  },
  async getSettings(): Promise<DDNSSettingsPayload> {
    const res = await apiClient.get("/ddns/settings");
    return res.data.data;
  },
  async saveSettings(
    payload: DDNSSettingsUpdatePayload,
  ): Promise<DDNSSettingsPayload> {
    const res = await apiClient.post("/ddns/settings", payload);
    return res.data.data;
  },
  async testPublicCheckSources(
    publicCheckSources: DDNSPublicCheckSourcesPayload,
    options: {
      httpTransport?: DDNSHttpTransport;
      publicDnsProvider?: DDNSPublicDnsProvider;
      networkInterface?: string;
    } = {},
  ): Promise<{ results: DDNSPublicCheckTestResultPayload[] }> {
    const res = await apiClient.post("/ddns/public-check/test", {
      publicCheckSources,
      ...options,
    });
    return res.data.data;
  },
  async getProviders(): Promise<
    Array<{
      name: string;
      label: string;
      fields: Array<{
        key: string;
        label: string;
        type: string;
        placeholder?: string;
        required?: boolean;
        options?: Array<{ label: string; value: string }>;
        description?: string;
      }>;
      capabilities?: DDNSProviderCapabilities;
    }>
  > {
    const res = await apiClient.get("/ddns/providers");
    return res.data.data;
  },
  async getNetworkInterfaces(): Promise<DDNSNetworkInterfacePayload[]> {
    const res = await apiClient.get("/ddns/interfaces");
    return res.data.data;
  },
  async resolveInterfaceSelector(
    payload: {
      networkInterface: string;
      family: "ipv4" | "ipv6";
      selector: DDNSInterfaceSelector;
      currentAddress?: string | null;
    },
    signal?: AbortSignal,
  ): Promise<DDNSInterfaceSelectorPreviewPayload> {
    const res = await apiClient.post("/ddns/interfaces/resolve", payload, {
      signal,
    });
    return res.data.data;
  },
  async setProvider(provider: string): Promise<void> {
    await apiClient.post("/ddns/provider", { provider });
  },
  async getConfig(provider: string): Promise<Record<string, string>> {
    const res = await apiClient.get(
      `/ddns/config/${encodeURIComponent(provider)}`,
    );
    return res.data.data;
  },
  async saveConfig(
    provider: string,
    config: Record<string, string>,
  ): Promise<void> {
    await apiClient.post(`/ddns/config/${encodeURIComponent(provider)}`, {
      config,
    });
  },
  async test(): Promise<{
    success: boolean;
    message: string;
    data?: {
      ipv4: string | null;
      ipv6: string | null;
      source?: DDNSIpSource;
      sourceLabel?: string;
    };
  }> {
    const res = await apiClient.post("/ddns/test");
    return res.data;
  },
  async getTargets(): Promise<DDNSTargetListPayload> {
    const res = await apiClient.get("/ddns/targets");
    return res.data.data;
  },
  async getTarget(id: string): Promise<DDNSTargetDetailPayload> {
    const res = await apiClient.get(`/ddns/targets/${encodeURIComponent(id)}`);
    return res.data.data;
  },
  async createTarget(payload: {
    name?: string;
    provider: string;
    enabled?: boolean;
    config: Record<string, string>;
  }): Promise<DDNSTargetDetailPayload> {
    const res = await apiClient.post("/ddns/targets", payload);
    return res.data.data;
  },
  async updateTarget(
    id: string,
    payload: {
      name?: string;
      provider: string;
      enabled?: boolean;
      config: Record<string, string>;
    },
  ): Promise<DDNSTargetDetailPayload> {
    const res = await apiClient.put(
      `/ddns/targets/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data.data;
  },
  async deleteTarget(id: string): Promise<void> {
    await apiClient.delete(`/ddns/targets/${encodeURIComponent(id)}`);
  },
  async setTargetEnabled(id: string, enabled: boolean): Promise<void> {
    await apiClient.post(`/ddns/targets/${encodeURIComponent(id)}/enabled`, {
      enabled,
    });
  },
  async testTarget(id: string): Promise<{
    success: boolean;
    message: string;
    data?: {
      ipv4: string | null;
      ipv6: string | null;
      source?: DDNSIpSource;
      sourceLabel?: string;
    };
  }> {
    const res = await apiClient.post(
      `/ddns/targets/${encodeURIComponent(id)}/test`,
    );
    return res.data;
  },
  async getLogs(limit = 200): Promise<DDNSLogEntry[]> {
    const res = await apiClient.get("/ddns/logs", { params: { limit } });
    return res.data.data;
  },
  async clearLogs(): Promise<void> {
    await apiClient.delete("/ddns/logs");
  },
  async poll(cursor?: number): Promise<DDNSPollPayload> {
    const res = await apiClient.get("/ddns/poll", {
      params: typeof cursor === "number" ? { cursor } : undefined,
    });
    return res.data.data;
  },
};
