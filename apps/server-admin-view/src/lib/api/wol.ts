import { apiClient } from "./client";

export type WOLRelay = {
  id: string;
  name: string;
  address: string;
  port: number;
  enabled: boolean;
  keyVersion: number;
  pskConfigured: boolean;
  createdAt: string;
  updatedAt: string;
};

export type WOLRelaySummary = Pick<
  WOLRelay,
  "id" | "name" | "address" | "port" | "enabled" | "pskConfigured"
>;

export type WOLTarget = {
  id: string;
  name: string;
  mac: string;
  relayId: string | null;
  broadcastAddress: string | null;
  ipAddress: string | null;
  deliveryMode: "local" | "relay";
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
  relay: WOLRelaySummary | null;
  status: WOLTargetStatus;
  integrations: WOLTargetIntegrations;
};

export type WOLIntegrationRuntimeState =
  | "disabled"
  | "credential_missing"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "error";

export type WOLIntegrationRuntime = {
  state: WOLIntegrationRuntimeState;
  lastConnectedAt: string | null;
  lastMessageAt: string | null;
  lastError: string | null;
};

export type WOLTargetIntegrations = {
  blinker: {
    enabled: boolean;
    bindComponent: boolean;
    skipTlsVerify: boolean;
    credentialConfigured: boolean;
    runtime: WOLIntegrationRuntime;
  };
  bemfa: {
    enabled: boolean;
    topic: string;
    skipTlsVerify: boolean;
    credentialConfigured: boolean;
    runtime: WOLIntegrationRuntime;
  };
};

export type WOLTargetIntegrationInput = {
  blinker: {
    enabled: boolean;
    deviceKey?: string;
    bindComponent: boolean;
    skipTlsVerify: boolean;
  };
  bemfa: {
    enabled: boolean;
    privateKey?: string;
    topic: string;
    skipTlsVerify: boolean;
  };
};

export type WOLTargetStatus = {
  state: "online" | "offline" | "unknown";
  checkedAt: string | null;
  lastOnlineAt: string | null;
  observedIp: string | null;
  lastError: string | null;
};

export type WOLRelayInput = Pick<
  WOLRelay,
  "name" | "address" | "port" | "enabled"
>;

export type WOLTargetInput = Pick<
  WOLTarget,
  "name" | "mac" | "relayId" | "broadcastAddress" | "ipAddress" | "enabled"
> & { integrations?: WOLTargetIntegrationInput };

export type WOLBootstrap = {
  pairingCode: string;
};

export type WOLRelayCredentialResult = {
  relay: WOLRelay;
  bootstrap: WOLBootstrap;
};

export type WOLDispatchResult = {
  requestId: string;
  relayId: string | null;
  deliveryMode: "local" | "relay";
  targetId?: string;
  status: "ready" | "broadcasted";
  attempts: number;
  latencyMs: number;
  acknowledgedAt: string;
};

export type WOLLocalRelayConfig = {
  enabled: boolean;
  relayId: string;
  keyVersion: number;
  listenAddress: string;
  port: number;
  broadcastDestinations: string[];
  allowedSources: string[];
  pskConfigured: boolean;
  updatedAt: string;
};

export type WOLLocalRelayRuntime = {
  enabled: boolean;
  active: boolean;
  listenAddress: string | null;
  lastError: string | null;
  updatedAt: string | null;
};

export type WOLLocalRelay = {
  config: WOLLocalRelayConfig;
  runtime: WOLLocalRelayRuntime;
};

export type WOLLocalRelayInput = Omit<
  WOLLocalRelayConfig,
  "pskConfigured" | "updatedAt"
> & { psk?: string };

export type WOLLocalNetwork = {
  interfaceName: string;
  address: string;
  cidr: string;
  scanCidr: string;
  broadcastAddress: string;
};

export type WOLDiscoveredDevice = {
  ip: string;
  mac: string;
  interfaceName: string;
  broadcastAddress: string;
};

export type WOLDiscoveryResult = {
  devices: WOLDiscoveredDevice[];
  networks: WOLLocalNetwork[];
  durationMs: number;
  method: string;
};

export type WOLDiscoveryProgress = {
  scannedHosts: number;
  totalHosts: number;
  foundDevices: number;
  currentHost: string;
};

export type WOLDiscoveryJobState =
  "queued" | "running" | "completed" | "cancelled" | "failed";

export type WOLDiscoveryJobStatus = {
  jobId: string;
  state: WOLDiscoveryJobState;
  createdAt: number;
  updatedAt: number;
  networks: WOLLocalNetwork[];
  progress: WOLDiscoveryProgress;
  devices: WOLDiscoveredDevice[];
  nextCursor: number;
  result: WOLDiscoveryResult | null;
  error: string | null;
};

export type WOLDiscoveryPollEvent =
  | { type: "meta"; data: WOLDiscoveryJobStatus }
  | { type: "progress"; data: WOLDiscoveryProgress }
  | { type: "device"; data: WOLDiscoveredDevice }
  | { type: "done"; data: WOLDiscoveryResult }
  | { type: "cancelled" };

export type WOLDiscoveryPollOptions = {
  signal?: AbortSignal;
  intervalMs?: number;
  onEvent?: (event: WOLDiscoveryPollEvent) => void;
};

const createDiscoveryAbortError = () => {
  const error = new Error("Discovery cancelled");
  error.name = "AbortError";
  return error;
};

const throwIfDiscoveryAborted = (signal?: AbortSignal) => {
  if (signal?.aborted) throw createDiscoveryAbortError();
};

const waitForDiscoveryPoll = (ms: number, signal?: AbortSignal) =>
  new Promise<void>((resolve, reject) => {
    if (signal?.aborted) {
      reject(createDiscoveryAbortError());
      return;
    }
    const timer = globalThis.setTimeout(() => {
      signal?.removeEventListener("abort", onAbort);
      resolve();
    }, ms);
    const onAbort = () => {
      globalThis.clearTimeout(timer);
      signal?.removeEventListener("abort", onAbort);
      reject(createDiscoveryAbortError());
    };
    signal?.addEventListener("abort", onAbort, { once: true });
  });

type WOLList<T> = { total: number; items: T[] };

export const WOLAPI = {
  async getLocalRelay(): Promise<WOLLocalRelay> {
    const response = await apiClient.get("/wol/local-relay");
    return response.data.data;
  },
  async updateLocalRelay(payload: WOLLocalRelayInput): Promise<WOLLocalRelay> {
    const response = await apiClient.put("/wol/local-relay", payload);
    return response.data.data;
  },
  async pairLocalRelay(pairingCode: string): Promise<WOLLocalRelay> {
    const response = await apiClient.post("/wol/local-relay/pair", {
      pairingCode,
    });
    return response.data.data;
  },
  async listRelays(): Promise<WOLList<WOLRelay>> {
    const response = await apiClient.get("/wol/relays");
    return response.data.data;
  },
  async createRelay(payload: WOLRelayInput): Promise<WOLRelayCredentialResult> {
    const response = await apiClient.post("/wol/relays", payload);
    return response.data.data;
  },
  async updateRelay(id: string, payload: WOLRelayInput): Promise<WOLRelay> {
    const response = await apiClient.put(
      `/wol/relays/${encodeURIComponent(id)}`,
      payload,
    );
    return response.data.data;
  },
  async deleteRelay(id: string): Promise<void> {
    await apiClient.delete(`/wol/relays/${encodeURIComponent(id)}`);
  },
  async rotateRelayPsk(id: string): Promise<WOLRelayCredentialResult> {
    const response = await apiClient.post(
      `/wol/relays/${encodeURIComponent(id)}/rotate-psk`,
    );
    return response.data.data;
  },
  async probeRelay(id: string): Promise<WOLDispatchResult> {
    const response = await apiClient.post(
      `/wol/relays/${encodeURIComponent(id)}/probe`,
    );
    return response.data.data;
  },
  async listTargets(): Promise<WOLList<WOLTarget>> {
    const response = await apiClient.get("/wol/targets");
    return response.data.data;
  },
  async getTarget(id: string, signal?: AbortSignal): Promise<WOLTarget> {
    const response = await apiClient.get(
      `/wol/targets/${encodeURIComponent(id)}`,
      { signal },
    );
    return response.data.data;
  },
  async startDiscoveryJob(
    targetCidrs: string[],
    signal?: AbortSignal,
  ): Promise<WOLDiscoveryJobStatus> {
    const response = await apiClient.post(
      "/wol/discover/jobs",
      { targetCidrs },
      { signal },
    );
    return response.data.data;
  },
  async getDiscoveryJob(
    jobId: string,
    cursor: number,
    signal?: AbortSignal,
  ): Promise<WOLDiscoveryJobStatus> {
    const response = await apiClient.get(
      `/wol/discover/jobs/${encodeURIComponent(jobId)}`,
      { params: { cursor }, signal },
    );
    return response.data.data;
  },
  async cancelDiscoveryJob(jobId: string): Promise<void> {
    await apiClient.delete(`/wol/discover/jobs/${encodeURIComponent(jobId)}`);
  },
  async discoverLocalDevices(
    targetCidrs: string[],
    options: WOLDiscoveryPollOptions = {},
  ): Promise<WOLDiscoveryResult> {
    let jobId = "";
    let cursor = 0;
    let cancelRequested = false;
    const requestCancel = () => {
      if (!jobId || cancelRequested) return;
      cancelRequested = true;
      void this.cancelDiscoveryJob(jobId).catch(() => undefined);
    };

    try {
      throwIfDiscoveryAborted(options.signal);
      const started = await this.startDiscoveryJob(targetCidrs, options.signal);
      jobId = started.jobId;
      options.onEvent?.({ type: "meta", data: started });

      while (true) {
        throwIfDiscoveryAborted(options.signal);
        const status = await this.getDiscoveryJob(
          jobId,
          cursor,
          options.signal,
        );
        options.onEvent?.({ type: "progress", data: status.progress });
        for (const device of status.devices) {
          options.onEvent?.({ type: "device", data: device });
        }
        cursor = status.nextCursor;

        if (status.state === "completed") {
          if (!status.result)
            throw new Error("Discovery completed without a result");
          options.onEvent?.({ type: "done", data: status.result });
          return status.result;
        }
        if (status.state === "cancelled") {
          options.onEvent?.({ type: "cancelled" });
          throw createDiscoveryAbortError();
        }
        if (status.state === "failed") {
          throw new Error(status.error || "LAN discovery failed");
        }
        await waitForDiscoveryPoll(options.intervalMs ?? 350, options.signal);
      }
    } catch (error) {
      if (options.signal?.aborted) {
        requestCancel();
        throw createDiscoveryAbortError();
      }
      throw error;
    }
  },
  async createTarget(payload: WOLTargetInput): Promise<WOLTarget> {
    const response = await apiClient.post("/wol/targets", payload);
    return response.data.data;
  },
  async updateTarget(id: string, payload: WOLTargetInput): Promise<WOLTarget> {
    const response = await apiClient.put(
      `/wol/targets/${encodeURIComponent(id)}`,
      payload,
    );
    return response.data.data;
  },
  async deleteTarget(id: string): Promise<void> {
    await apiClient.delete(`/wol/targets/${encodeURIComponent(id)}`);
  },
  async wakeTarget(id: string): Promise<WOLDispatchResult> {
    const response = await apiClient.post(
      `/wol/targets/${encodeURIComponent(id)}/wake`,
    );
    return response.data.data;
  },
};
