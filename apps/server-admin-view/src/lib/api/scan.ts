import type { components as ApiContractComponents } from "@fn-knock/api-contract";

import { apiClient } from "./client";

type ScanSchemas = ApiContractComponents["schemas"];

export type DiscoveredServiceInfo = ScanSchemas["ScanDiscoveredServiceData"];
export type ScanDiscoverResponse = ScanSchemas["ScanDiscoverResultData"];
export type ScanDiscoverMeta = ScanSchemas["ScanDiscoverMetaData"];
export type ScanIntensityMode = ScanDiscoverySettings["intensityMode"];
export type ScanIntensityLevel = ScanDiscoverySettings["effectiveLevel"];
export type ScanDiscoveryCapability =
  ScanSchemas["ScanDiscoveryCapabilityData"];
export type ScanDiscoverySettings = ScanSchemas["ScanDiscoverySettingsData"];
export type ScanDiscoverySettingsSaveRequest =
  ScanSchemas["ScanDiscoverySettingsUpdateData"];
export type ScanDiscoverProgress = ScanSchemas["ScanDiscoverProgressData"];

export type ScanDiscoverPollEvent =
  | {
      type: "meta";
      data: ScanDiscoverMeta;
    }
  | {
      type: "progress";
      data: ScanDiscoverProgress;
    }
  | {
      type: "service";
      data: {
        service: DiscoveredServiceInfo;
      };
    }
  | {
      type: "done";
      data: ScanDiscoverResponse;
    }
  | {
      type: "cancelled";
    };

export type ScanDiscoveryTarget = ScanSchemas["ScanDiscoveryTargetData"];
export type ScanDiscoveryTargetSource = ScanDiscoveryTarget["source"];
export type ScanDiscoveryHostCandidate =
  ScanSchemas["ScanDiscoveryHostCandidateData"];
export type ScanDiscoveryHostCandidateSource =
  ScanDiscoveryHostCandidate["source"];
export type ScanDiscoveryTargetsResponse =
  ScanSchemas["ScanDiscoveryTargetsData"];
export type ScanDiscoverRequest = ScanSchemas["ScanDiscoverJobBodyData"];

export interface ScanDiscoverPollOptions {
  signal?: AbortSignal;
  intervalMs?: number;
  onEvent?: (event: ScanDiscoverPollEvent) => void;
}

export type ScanDiscoverJobStatus = ScanSchemas["ScanDiscoverJobData"];
export type ScanDiscoverJobState = ScanDiscoverJobStatus["state"];

const createScanAbortError = (): Error => {
  const error = new Error("Scan cancelled");
  error.name = "AbortError";
  return error;
};

const throwIfScanAborted = (signal?: AbortSignal) => {
  if (signal?.aborted) {
    throw createScanAbortError();
  }
};

const waitForDiscoverPoll = (ms: number, signal?: AbortSignal) =>
  new Promise<void>((resolve, reject) => {
    if (signal?.aborted) {
      reject(createScanAbortError());
      return;
    }

    const timer = window.setTimeout(() => {
      signal?.removeEventListener("abort", onAbort);
      resolve();
    }, ms);
    const onAbort = () => {
      window.clearTimeout(timer);
      signal?.removeEventListener("abort", onAbort);
      reject(createScanAbortError());
    };
    signal?.addEventListener("abort", onAbort, { once: true });
  });

export type HostMappingProbeResult =
  ScanSchemas["HostMappingProbeResultData"];
export type HostMappingProbeStatus = HostMappingProbeResult["status"];
export type HostMappingsProbeRequest = ScanSchemas["HostMappingsProbeBodyData"];
export type HostMappingsProbeResponse = ScanSchemas["HostMappingsProbeData"];
export type ScanDiscoveryTargetsSaveRequest =
  ScanSchemas["ScanDiscoveryTargetsUpdateData"];

export const ScanAPI = {
  async getDiscoverSettings(): Promise<ScanDiscoverySettings> {
    const res = await apiClient.get("/scan/discover-settings");
    return res.data.data;
  },
  async saveDiscoverSettings(
    payload: ScanDiscoverySettingsSaveRequest,
  ): Promise<ScanDiscoverySettings> {
    const res = await apiClient.post("/scan/discover-settings", payload);
    return res.data.data;
  },
  async startDiscoverJob(
    payload: ScanDiscoverRequest,
    signal?: AbortSignal,
  ): Promise<ScanDiscoverJobStatus> {
    const res = await apiClient.post("/scan/discover/jobs", payload, {
      signal,
    });
    return res.data.data;
  },
  async getDiscoverJob(
    jobId: string,
    cursor = 0,
    signal?: AbortSignal,
  ): Promise<ScanDiscoverJobStatus> {
    const res = await apiClient.get(
      `/scan/discover/jobs/${encodeURIComponent(jobId)}`,
      {
        params: { cursor },
        signal,
      },
    );
    return res.data.data;
  },
  async cancelDiscoverJob(jobId: string): Promise<ScanDiscoverJobStatus> {
    const res = await apiClient.delete(
      `/scan/discover/jobs/${encodeURIComponent(jobId)}`,
    );
    return res.data.data;
  },
  async discoverPolling(
    payload: ScanDiscoverRequest,
    options: ScanDiscoverPollOptions = {},
  ): Promise<ScanDiscoverResponse> {
    let jobId = "";
    let cursor = 0;
    let hasEmittedMeta = false;
    let hasRequestedCancel = false;
    let removeAbortListener: (() => void) | null = null;
    const intervalMs = options.intervalMs ?? 700;
    const requestCancel = () => {
      if (!jobId || hasRequestedCancel) return;
      hasRequestedCancel = true;
      void this.cancelDiscoverJob(jobId).catch(() => undefined);
    };

    try {
      throwIfScanAborted(options.signal);
      const started = await this.startDiscoverJob(payload, options.signal);
      jobId = started.jobId;
      if (options.signal) {
        const onAbort = () => requestCancel();
        options.signal.addEventListener("abort", onAbort, { once: true });
        removeAbortListener = () =>
          options.signal?.removeEventListener("abort", onAbort);
      }
      throwIfScanAborted(options.signal);

      while (true) {
        throwIfScanAborted(options.signal);
        const status = await this.getDiscoverJob(jobId, cursor, options.signal);

        if (status.meta && !hasEmittedMeta) {
          hasEmittedMeta = true;
          options.onEvent?.({ type: "meta", data: status.meta });
        }

        if (status.progress) {
          options.onEvent?.({ type: "progress", data: status.progress });
        }

        for (const service of status.services) {
          options.onEvent?.({
            type: "service",
            data: { service },
          });
        }
        cursor = status.nextCursor;

        if (status.state === "completed") {
          if (!status.result) {
            throw new Error("Scan job completed without a result");
          }
          removeAbortListener?.();
          removeAbortListener = null;
          options.onEvent?.({ type: "done", data: status.result });
          return status.result;
        }

        if (status.state === "cancelled") {
          options.onEvent?.({ type: "cancelled" });
          throw createScanAbortError();
        }

        if (status.state === "failed") {
          throw new Error(status.error || "Scan failed");
        }

        await waitForDiscoverPoll(intervalMs, options.signal);
      }
    } catch (error) {
      if (jobId && options.signal?.aborted) {
        requestCancel();
      }
      if (options.signal?.aborted) {
        throw createScanAbortError();
      }
      throw error;
    } finally {
      removeAbortListener?.();
    }
  },
  async probeHostMappings(
    payload?: HostMappingsProbeRequest,
  ): Promise<HostMappingsProbeResponse> {
    const res = await apiClient.post(
      "/scan/host-mappings/probe",
      payload || {},
    );
    return res.data.data;
  },
  async getDiscoverTargets(): Promise<ScanDiscoveryTargetsResponse> {
    const res = await apiClient.get("/scan/discover-targets");
    return res.data.data;
  },
  async saveDiscoverTargets(
    payload: ScanDiscoveryTargetsSaveRequest,
  ): Promise<ScanDiscoveryTargetsResponse> {
    const res = await apiClient.post("/scan/discover-targets", payload);
    return res.data.data;
  },
};
