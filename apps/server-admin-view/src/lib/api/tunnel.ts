import { apiClient } from "./client";

export type FrpcTcpItem = {
  name: string;
  type: string;
  status: string;
  err: string;
  local_addr: string;
  plugin: string;
  remote_addr: string;
};

export type FrpcInstanceSummary = {
  serverAddr: string;
  serverPort: string;
  localPort: string;
  remotePort: string;
};

export type TunnelSupervisorFailure = {
  at: string;
  startedAt: string | null;
  reason: string;
  exitCode: number | null;
  signal: number | null;
  coreDumped: boolean;
  uptimeMs: number;
  diagnosis: string | null;
};

export type TunnelSupervisorStatus = {
  state: "stopped" | "starting" | "running" | "backoff";
  desiredRunning: boolean;
  running: boolean;
  attached: boolean;
  pid: number | null;
  restartCount: number;
  consecutiveFailures: number;
  nextRestartAt: string | null;
  startedAt: string | null;
  stoppedAt: string | null;
  lastFailure: TunnelSupervisorFailure | null;
  lastMessage: string | null;
};

export type FrpcInstanceStatus = {
  id: string;
  name: string;
  isPrimary: boolean;
  configPath: string;
  workDir: string;
  createdAt: string;
  updatedAt: string;
  sortOrder: number;
  desiredRunning: boolean;
  running: boolean;
  attached: boolean;
  pid: number | null;
  startedAt: string | null;
  stoppedAt: string | null;
  lastExitCode: number | null;
  lastMessage: string | null;
  supervisor: TunnelSupervisorStatus;
  summary: FrpcInstanceSummary;
};

export type FrpcInstancesOverview = {
  initialized: boolean;
  platform: string;
  primaryInstanceId: string;
  total: number;
  extraCount: number;
  runningCount: number;
  defaults: { local_port: string };
  items: FrpcInstanceStatus[];
};

export type FrpcInstanceDetail = {
  item: FrpcInstanceStatus;
  content: string;
  logs: string[];
};

export type FrpcStatusPayload = FrpcInstanceStatus & {
  tcp: FrpcTcpItem[];
  instances?: FrpcInstancesOverview;
};

export type FrpcPollPayload = {
  cursor: number;
  reset: boolean;
  logs: string[];
  status: FrpcStatusPayload;
};

export type FrpcInstancePollPayload = {
  cursor: number;
  reset: boolean;
  logs: string[];
  status: FrpcInstanceStatus;
};

export type CloudflaredProtocol = "auto" | "http2" | "quic";

export type CloudflaredConfig = {
  mode: "manual" | "managed";
  protocol: CloudflaredProtocol;
  apiTokenConfigured: boolean;
  tunnelTokenConfigured: boolean;
  accountId: string | null;
  zoneId: string | null;
  zoneName: string | null;
  tunnel: CloudflareTunnelSummary | null;
  optimizationEnabled: boolean;
};

export type CloudflareTunnelSummary = {
  id: string;
  name: string;
  status?: string | null;
  connections?: number;
  ownership?: "dedicated" | "adopted";
};

export type CloudflareOptimizationCandidate = {
  ip: string;
  medianLatencyMs: number;
  jitterMs: number;
  lossRatio: number;
  downloadMbps: number;
  score: number;
  verifiedAt?: string | null;
  sourceTypes: Array<"official-range" | "builtin" | "custom" | string>;
  sourceHostnames: string[];
  colo: string | null;
  cfRay: string | null;
  businessHostname: string | null;
  businessStatus: number | null;
  businessColo: string | null;
  businessCfRay: string | null;
  businessValidated: boolean;
};

export type CloudflareOptimizationVantage = {
  id: string;
  label: string;
  publicIp: string | null;
  defaultColo: string | null;
  measuredAt: string;
};

export type CloudflareOptimizationCandidateSources = {
  officialRanges: boolean;
  builtins: Array<{
    id: string;
    hostname: string;
    category: string;
    enabled: boolean;
  }>;
  customHostnames: string[];
  maxCustomHostnames: number;
  resolutionPolicy: string;
  publishPolicy: string;
  error?: string | null;
};

export type CloudflareOptimizationScan = {
  id: string;
  status: "queued" | "running" | "completed" | "failed" | "cancelled";
  phase: string;
  progress: number;
  createdAt: string;
  startedAt: string | null;
  completedAt: string | null;
  completedAtMs?: number | null;
  cancelRequested: boolean;
  candidates: CloudflareOptimizationCandidate[];
  recommendedIp: string | null;
  vantage: CloudflareOptimizationVantage | null;
  sourceWarnings: string[];
  candidateSourceCount?: number;
  businessValidationHostname?: string | null;
  sourceFingerprint?: string | null;
  errorCode?: string | null;
  error: string | null;
};

export type CloudflareOptimizationDomain = {
  hostname: string;
  status: string;
  managementMode?: "optimize" | "external";
  sslStatus: string | null;
  customHostnameId: string | null;
  optimized: boolean;
  actionRequired?: boolean;
  cleanupPending?: boolean;
  conflictResourceId?: string | null;
  messageCode?: string | null;
  messageDetail?: string | null;
  message: string | null;
};

export type CloudflareManagedState = {
  mode: "manual" | "managed";
  apiTokenConfigured: boolean;
  tunnelTokenConfigured: boolean;
  connection: {
    accountId: string | null;
    zoneId: string | null;
    zoneName: string | null;
    configuredRootDomain: string;
    rootDomainDrift: boolean;
    remoteError: string | null;
  };
  tunnels: CloudflareTunnelSummary[];
  managed: {
    tunnel?: CloudflareTunnelSummary;
    wildcardDns?: { id: string; name: string; content: string };
    ingress?: { hostname: string };
    updatedAt?: string;
  };
  optimization: {
    enabled: boolean;
    beta: boolean;
    ipv4Only: boolean;
    selected:
      | (CloudflareOptimizationCandidate & {
          selectedAt?: string;
          source?: string;
        })
      | null;
    fallbackActive: boolean;
    publishSuppressed: boolean;
    originHostname: string | null;
    edgeHostname: string | null;
    fallbackOrigin: {
      origin: string;
      status: string;
      errors?: string[];
      ownership?: "dedicated" | "adopted";
      updatedAt?: string;
    } | null;
    capabilityProbe: {
      status: "pending" | "awaiting-candidate" | "compatible" | "unsupported";
      hostname?: string;
      hostnameStatus?: string;
      sslStatus?: string;
      testedIp?: string;
      testedAt?: string;
      reasonCode?: string;
      message?: string;
    } | null;
    scanReady: boolean;
    scanReadinessErrorCode: string | null;
    candidateSources: CloudflareOptimizationCandidateSources;
    vantage: CloudflareOptimizationVantage | null;
    sourceWarnings: string[];
    domains: CloudflareOptimizationDomain[];
    schedule: {
      fullScanIntervalDays: number;
      healthCheckIntervalMinutes: number;
      nextFullScanAt: string | null;
      lastFullScanAt: string | null;
      lastHealthAt: string | null;
      healthFailures: number;
      lastSwitchReason: string | null;
      lastError: string | null;
    };
    scans: CloudflareOptimizationScan[];
  };
  permissions: string[];
};

export type CloudflareReconcileOperation = {
  id: string;
  kind: string;
  action:
    | "create"
    | "update"
    | "delete"
    | "keep"
    | "keep-deleted"
    | "fallback"
    | "probe"
    | "recover";
  target: string;
  owned: boolean;
};

export type CloudflareReconcileConflict = {
  id: string;
  kind: string;
  target: string;
  messageCode?: string;
  detail?: string;
  message: string;
  takeoverAllowed: boolean;
  details?: {
    records: Array<{
      type: string | null;
      content: string | null;
      proxied: boolean | null;
      ownerKind: "current-instance" | "other-fn-knock-instance" | "external";
    }>;
    desired: {
      type: string;
      content: string;
      proxied: boolean;
    };
  };
};

export type CloudflareReconcilePlan = {
  planId: string;
  expiresAt: string;
  action: "apply" | "cleanup";
  rootDomain: string;
  accountId: string;
  zoneId: string;
  selectedTunnelId: string | null;
  remoteFingerprint: string;
  capabilities: Record<
    "zoneRead" | "tunnelEdit" | "dnsEdit" | "sslCertificatesEdit",
    {
      required: boolean;
      readable: boolean | null;
      writeVerified: boolean | null;
    }
  >;
  operations: CloudflareReconcileOperation[];
  conflicts: CloudflareReconcileConflict[];
  warnings: string[];
  warningCodes?: string[];
  canApply: boolean;
};

export type CloudflaredStatusPayload = {
  running: boolean;
  pid: number | null;
  desiredRunning: boolean;
  supervisor: TunnelSupervisorStatus;
};

export type CloudflaredPollPayload = {
  cursor: number;
  reset: boolean;
  logs: string[];
  status: CloudflaredStatusPayload;
};

export const FrpcAPI = {
  async getStatus(): Promise<{
    initialized: boolean;
    platform: string;
    running: boolean;
    pid: number | null;
    desiredRunning: boolean;
    supervisor: TunnelSupervisorStatus;
    config_path: string;
    defaults: { local_port: string };
  }> {
    const res = await apiClient.get("/frpc/status");
    return res.data.data;
  },
  async getOverview(
    limit = 200,
  ): Promise<{ tcp: FrpcTcpItem[]; logs: string[] }> {
    const res = await apiClient.get("/frpc/overview", { params: { limit } });
    return res.data.data;
  },
  async getWebStatus(): Promise<{ tcp: FrpcTcpItem[] }> {
    const res = await apiClient.get("/frpc/web-status");
    return res.data.data;
  },
  async getConfig(): Promise<string> {
    const res = await apiClient.get("/frpc/config");
    return res.data.data.content as string;
  },
  async saveConfig(content: string): Promise<void> {
    await apiClient.post("/frpc/config", { content });
  },
  async start(): Promise<{ pid: number }> {
    const res = await apiClient.post("/frpc/start");
    return res.data.data;
  },
  async stop(): Promise<void> {
    await apiClient.post("/frpc/stop");
  },
  async getLogs(limit = 200): Promise<string[]> {
    const res = await apiClient.get("/frpc/logs", { params: { limit } });
    return res.data.data as string[];
  },
  async clearLogs(): Promise<void> {
    await apiClient.delete("/frpc/logs");
  },
  async poll(cursor?: number): Promise<FrpcPollPayload> {
    const res = await apiClient.get("/frpc/poll", {
      params: typeof cursor === "number" ? { cursor } : undefined,
    });
    return res.data.data;
  },
  async getInstances(): Promise<FrpcInstancesOverview> {
    const res = await apiClient.get("/frpc/instances");
    return res.data.data;
  },
  async createDraft(): Promise<string> {
    const res = await apiClient.post("/frpc/instances/draft");
    return res.data.data.content as string;
  },
  async createInstance(payload: {
    name?: string;
    content?: string;
  }): Promise<FrpcInstanceStatus> {
    const res = await apiClient.post("/frpc/instances", payload);
    return res.data.data;
  },
  async getInstance(id: string, limit = 200): Promise<FrpcInstanceDetail> {
    const res = await apiClient.get(
      `/frpc/instances/${encodeURIComponent(id)}`,
      { params: { limit } },
    );
    return res.data.data;
  },
  async updateInstance(
    id: string,
    payload: { name?: string; content?: string },
  ): Promise<FrpcInstanceStatus> {
    const res = await apiClient.put(
      `/frpc/instances/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data.data;
  },
  async deleteInstance(id: string): Promise<void> {
    await apiClient.delete(`/frpc/instances/${encodeURIComponent(id)}`);
  },
  async startInstance(id: string): Promise<{ pid: number }> {
    const res = await apiClient.post(
      `/frpc/instances/${encodeURIComponent(id)}/start`,
    );
    return res.data.data;
  },
  async stopInstance(id: string): Promise<void> {
    await apiClient.post(`/frpc/instances/${encodeURIComponent(id)}/stop`);
  },
  async restartInstance(id: string): Promise<{ pid: number }> {
    const res = await apiClient.post(
      `/frpc/instances/${encodeURIComponent(id)}/restart`,
    );
    return res.data.data;
  },
  async getInstanceLogs(id: string, limit = 200): Promise<string[]> {
    const res = await apiClient.get(
      `/frpc/instances/${encodeURIComponent(id)}/logs`,
      { params: { limit } },
    );
    return res.data.data as string[];
  },
  async clearInstanceLogs(id: string): Promise<void> {
    await apiClient.delete(`/frpc/instances/${encodeURIComponent(id)}/logs`);
  },
  async pollInstance(
    id: string,
    cursor?: number,
  ): Promise<FrpcInstancePollPayload> {
    const res = await apiClient.get(
      `/frpc/instances/${encodeURIComponent(id)}/poll`,
      { params: typeof cursor === "number" ? { cursor } : undefined },
    );
    return res.data.data;
  },
};

export const CloudflaredAPI = {
  async getStatus(): Promise<{
    initialized: boolean;
    platform: string;
    running: boolean;
    pid: number | null;
    desiredRunning: boolean;
    supervisor: TunnelSupervisorStatus;
  }> {
    const res = await apiClient.get("/cloudflared/status");
    return res.data.data;
  },
  async getConfig(): Promise<CloudflaredConfig> {
    const res = await apiClient.get("/cloudflared/config");
    return res.data.data;
  },
  async saveConfig(config: {
    protocol: CloudflaredProtocol;
    token?: string;
    clearToken?: boolean;
  }): Promise<void> {
    await apiClient.post("/cloudflared/config", config);
  },
  async saveCloudflareCredential(
    apiToken: string,
  ): Promise<CloudflareManagedState> {
    const res = await apiClient.put("/cloudflared/cloudflare/credential", {
      apiToken,
    });
    return res.data.data;
  },
  async deleteCloudflareCredential(): Promise<void> {
    await apiClient.delete("/cloudflared/cloudflare/credential");
  },
  async getCloudflareState(): Promise<CloudflareManagedState> {
    const res = await apiClient.get("/cloudflared/cloudflare/state");
    return res.data.data;
  },
  async previewReconcile(payload: {
    action?: "apply" | "cleanup";
    tunnelMode: "dedicated" | "existing";
    tunnelId?: string;
    optimizationEnabled: boolean;
    deleteDedicatedTunnel?: boolean;
  }): Promise<CloudflareReconcilePlan> {
    const res = await apiClient.post("/cloudflared/reconcile/preview", payload);
    return res.data.data;
  },
  async applyReconcile(payload: {
    planId: string;
    takeoverResourceIds?: string[];
  }): Promise<CloudflareManagedState> {
    const res = await apiClient.post("/cloudflared/reconcile/apply", payload);
    return res.data.data;
  },
  async startOptimizationScan(): Promise<CloudflareOptimizationScan> {
    const res = await apiClient.post("/cloudflared/optimization/scans");
    return res.data.data;
  },
  async saveOptimizationSourceSettings(payload: {
    officialRanges: boolean;
    builtinIds: string[];
    customHostnames: string[];
  }): Promise<CloudflareOptimizationCandidateSources> {
    const res = await apiClient.put(
      "/cloudflared/optimization/settings",
      payload,
    );
    return res.data.data;
  },
  async setOptimizationDomainMode(
    hostname: string,
    mode: "optimize" | "external",
  ): Promise<{
    hostname: string;
    mode: "optimize" | "external";
    cleanupPending: boolean;
  }> {
    const res = await apiClient.put(
      `/cloudflared/optimization/domains/${encodeURIComponent(hostname)}`,
      { mode },
    );
    return res.data.data;
  },
  async getOptimizationScan(id: string): Promise<CloudflareOptimizationScan> {
    const res = await apiClient.get(
      `/cloudflared/optimization/scans/${encodeURIComponent(id)}`,
    );
    return res.data.data;
  },
  async cancelOptimizationScan(id: string): Promise<void> {
    await apiClient.delete(
      `/cloudflared/optimization/scans/${encodeURIComponent(id)}`,
    );
  },
  async applyOptimization(payload: {
    scanId: string;
    candidateIp?: string;
  }): Promise<{
    selected: CloudflareOptimizationCandidate;
    state: unknown;
  }> {
    const res = await apiClient.post(
      "/cloudflared/optimization/apply",
      payload,
    );
    return res.data.data;
  },
  async fallbackOptimization(): Promise<{ fallbackActive: boolean }> {
    const res = await apiClient.post("/cloudflared/optimization/fallback");
    return res.data.data;
  },
  async start(): Promise<{ pid: number }> {
    const res = await apiClient.post("/cloudflared/start");
    return res.data.data;
  },
  async stop(): Promise<void> {
    await apiClient.post("/cloudflared/stop");
  },
  async getLogs(limit = 200): Promise<string[]> {
    const res = await apiClient.get("/cloudflared/logs", { params: { limit } });
    return res.data.data as string[];
  },
  async clearLogs(): Promise<void> {
    await apiClient.delete("/cloudflared/logs");
  },
  async poll(cursor?: number): Promise<CloudflaredPollPayload> {
    const res = await apiClient.get("/cloudflared/poll", {
      params: typeof cursor === "number" ? { cursor } : undefined,
    });
    return res.data.data;
  },
};
