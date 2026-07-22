import { apiClient } from "./client";

export interface DiscoveredServiceInfo {
  serviceKey?: string;
  host?: string;
  port: number;
  httpStatus: number;
  requiresBasicAuth?: boolean;
  detail: {
    name: string;
    label: string;
    rule: {
      path: string;
      rewrite_html: boolean;
      use_auth: boolean;
      use_root_mode: boolean;
      strip_path: boolean;
      target: string;
    };
    isDefault: boolean;
  };
}

export interface ScanDiscoverResponse {
  host: string;
  totalPortsScanned: number;
  foundServices: number;
  scannedHosts?: number;
  scanHostCount?: number;
  scanScope?: string | null;
  scanCidrs?: string[];
  intensityMode?: ScanIntensityMode;
  intensityLevel?: ScanIntensityLevel;
  recommendedLevel?: ScanIntensityLevel;
  configuredConcurrency?: number;
  effectiveConcurrency?: number;
  services: DiscoveredServiceInfo[];
}

export type ScanIntensityMode = "auto" | "manual";
export type ScanIntensityLevel = "low" | "medium" | "high" | "extreme";

export interface ScanDiscoveryCapability {
  cpuCores: number;
  totalMemoryMiB: number | null;
  availableMemoryMiB: number | null;
  fileDescriptorLimit: number | null;
  safeConcurrency: number;
}

export interface ScanDiscoverySettings {
  intensityMode: ScanIntensityMode;
  configuredLevel: ScanIntensityLevel;
  recommendedLevel: ScanIntensityLevel;
  effectiveLevel: ScanIntensityLevel;
  configuredConcurrency: number;
  effectiveConcurrency: number;
  capability: ScanDiscoveryCapability;
}

export interface ScanDiscoverySettingsSaveRequest {
  intensity_mode: ScanIntensityMode;
  intensity_level: ScanIntensityLevel;
}

export interface ScanDiscoverProgress {
  scannedPorts: number;
  totalPorts: number;
  scannedHosts: number;
  totalHosts: number;
  currentHost?: string;
}

export type ScanDiscoverPollEvent =
  | {
      type: "meta";
      data: ScanDiscoverResponse & {
        portRange?: string;
      };
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

export type ScanDiscoveryTargetSource =
  | "docker"
  | "loopback"
  | "interface"
  | "mapping"
  | "custom"
  | "saved";

export interface ScanDiscoveryTarget {
  cidr: string;
  label: string;
  source: ScanDiscoveryTargetSource;
  hostCount: number;
  isAutomatic: boolean;
}

export type ScanDiscoveryHostCandidateSource =
  | "configured"
  | "proxy"
  | "request_host";

export interface ScanDiscoveryHostCandidate {
  address: string;
  cidr: string;
  source: ScanDiscoveryHostCandidateSource;
  recommended: boolean;
  includedInAutomaticScan: boolean;
}

export interface ScanDiscoveryTargetsResponse {
  automaticTargets: ScanDiscoveryTarget[];
  hostCandidates?: ScanDiscoveryHostCandidate[];
  customTargets: ScanDiscoveryTarget[];
  selectedTargets?: ScanDiscoveryTarget[];
  selectionMode?: "automatic" | "custom";
  selectedCidrs: string[];
  effectiveCidrs: string[];
  limits: {
    maxCidrs: number;
    maxHosts: number;
  };
}

export interface ScanDiscoverRequest {
  target_cidrs: string[];
}

export interface ScanDiscoverPollOptions {
  signal?: AbortSignal;
  intervalMs?: number;
  onEvent?: (event: ScanDiscoverPollEvent) => void;
}

export type ScanDiscoverJobState =
  | "queued"
  | "running"
  | "completed"
  | "cancelled"
  | "failed";

export interface ScanDiscoverJobStatus {
  jobId: string;
  state: ScanDiscoverJobState;
  createdAt: number;
  updatedAt: number;
  meta: (ScanDiscoverResponse & { portRange?: string }) | null;
  progress: ScanDiscoverProgress | null;
  services: DiscoveredServiceInfo[];
  nextCursor: number;
  result: ScanDiscoverResponse | null;
  error: string | null;
}

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

export type HostMappingProbeStatus = "online" | "stale" | "unsupported";

export interface HostMappingProbeResult {
  host: string;
  target: string;
  status: HostMappingProbeStatus;
  httpStatus?: number;
  error?: string;
  latencyMs?: number;
}

export interface HostMappingsProbeRequest {
  hosts?: string[];
}

export interface HostMappingsProbeResponse {
  results: HostMappingProbeResult[];
}

export interface ScanDiscoveryTargetsSaveRequest {
  custom_cidrs?: string[];
  selected_cidrs?: string[];
}

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
