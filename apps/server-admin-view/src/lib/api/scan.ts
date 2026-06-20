import { apiClient } from "./client";

export interface DiscoveredServiceInfo {
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
  services: DiscoveredServiceInfo[];
}

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

export interface ScanDiscoveryTargetsResponse {
  automaticTargets: ScanDiscoveryTarget[];
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
  target_cidrs?: string[];
}

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
  async discover(payload?: ScanDiscoverRequest): Promise<ScanDiscoverResponse> {
    const res =
      payload && "target_cidrs" in payload
        ? await apiClient.post("/scan/discover", payload)
        : await apiClient.get("/scan/discover");
    return res.data.data;
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
