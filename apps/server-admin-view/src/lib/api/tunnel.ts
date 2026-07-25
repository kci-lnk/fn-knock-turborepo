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
  token: string;
  protocol: CloudflaredProtocol;
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
  async saveConfig(config: CloudflaredConfig): Promise<void> {
    await apiClient.post("/cloudflared/config", config);
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
