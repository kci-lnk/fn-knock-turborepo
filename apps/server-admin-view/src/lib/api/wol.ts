import type { components as ApiContractComponents } from "@fn-knock/api-contract";

import { apiClient } from "./client";

type WolSchemas = ApiContractComponents["schemas"];

export type WOLRelay = WolSchemas["WolRelayData"];
export type WOLRelaySummary = WolSchemas["WolRelaySummaryData"];
export type WOLTarget = WolSchemas["WolTargetData"];
export type WOLIntegrationRuntime =
  WolSchemas["WolIntegrationRuntimeData"];
export type WOLIntegrationRuntimeState = WOLIntegrationRuntime["state"];
export type WOLTargetIntegrations =
  WolSchemas["WolTargetIntegrationsData"];
type WOLBlinkerIntegrationInput =
  Omit<WolSchemas["WolBlinkerIntegrationInputData"], "deviceKey"> &
    Required<
      Pick<
        WolSchemas["WolBlinkerIntegrationInputData"],
        "bindComponent" | "skipTlsVerify"
      >
    > & { deviceKey?: string };
type WOLBemfaIntegrationInput =
  Omit<WolSchemas["WolBemfaIntegrationInputData"], "privateKey"> &
    Required<
      Pick<
        WolSchemas["WolBemfaIntegrationInputData"],
        "topic" | "skipTlsVerify"
      >
    > & { privateKey?: string };
export type WOLTargetIntegrationInput = {
  blinker: WOLBlinkerIntegrationInput;
  bemfa: WOLBemfaIntegrationInput;
};
export type WOLTargetStatus = WolSchemas["WolTargetStatusData"];
export type WOLRelayInput = WolSchemas["WolRelayInputData"] &
  Required<Pick<WolSchemas["WolRelayInputData"], "port" | "enabled">>;
export type WOLTargetInput = Omit<
  WolSchemas["WolTargetInputData"],
  "integrations"
> &
  Required<
    Pick<
      WolSchemas["WolTargetInputData"],
      "relayId" | "broadcastAddress" | "ipAddress" | "enabled"
    >
  > & { integrations?: WOLTargetIntegrationInput };
export type WOLBootstrap = WolSchemas["WolBootstrapData"];
export type WOLRelayCredentialResult =
  WolSchemas["WolRelayCredentialData"];
export type WOLDispatchResult = WolSchemas["WolDispatchData"];
export type WOLLocalRelayConfig = WolSchemas["WolLocalRelayConfigData"];
export type WOLLocalRelayRuntime = WolSchemas["WolLocalRelayRuntimeData"];
export type WOLLocalRelay = WolSchemas["WolLocalRelayData"];
export type WOLLocalRelayInput = WolSchemas["WolLocalRelayInputData"] &
  Required<Pick<WolSchemas["WolLocalRelayInputData"], "allowedSources">>;
export type WOLLocalNetwork = WolSchemas["WolLocalNetworkData"];
export type WOLDiscoveredDevice = WolSchemas["WolDiscoveredDeviceData"];
export type WOLDiscoveryResult = WolSchemas["WolDiscoveryResultData"];
export type WOLDiscoveryProgress = WolSchemas["WolDiscoveryProgressData"];
export type WOLDiscoveryJobStatus = WolSchemas["WolDiscoveryJobData"];
export type WOLDiscoveryJobState = WOLDiscoveryJobStatus["state"];

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

type WOLRelayList = WolSchemas["WolRelayListData"];
type WOLTargetList = WolSchemas["WolTargetListData"];
type WOLLocalRelayPairBody = WolSchemas["WolLocalRelayPairBodyData"];
type WOLDiscoveryBody = WolSchemas["WolDiscoveryBodyData"];

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
    const payload: WOLLocalRelayPairBody = {
      pairingCode,
    };
    const response = await apiClient.post("/wol/local-relay/pair", payload);
    return response.data.data;
  },
  async listRelays(): Promise<WOLRelayList> {
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
  async listTargets(): Promise<WOLTargetList> {
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
    const payload: WOLDiscoveryBody = { targetCidrs };
    const response = await apiClient.post(
      "/wol/discover/jobs",
      payload,
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
  async cancelDiscoveryJob(jobId: string): Promise<WOLDiscoveryJobStatus> {
    const response = await apiClient.delete(
      `/wol/discover/jobs/${encodeURIComponent(jobId)}`,
    );
    return response.data.data;
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
