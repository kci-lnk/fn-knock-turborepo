import type { components as ApiContractComponents } from "@fn-knock/api-contract";
import type { StreamMapping } from "../../types";
import { apiClient } from "./client";

type StreamMappingsUpdate =
  ApiContractComponents["schemas"]["StreamMappingsUpdateData"];

export type StreamServiceProfile =
  ApiContractComponents["schemas"]["StreamServiceProfileData"];
export type StreamBypassPolicy =
  ApiContractComponents["schemas"]["StreamBypassPolicyData"];
export type StreamServiceDescriptor = {
  service_id: string;
  display_name: string;
  service_family: string;
  transports: string[];
  active_probe_supported: boolean;
  strict_capable: boolean;
};
export type StreamServiceCatalog = {
  classifier_version: string;
  items: StreamServiceDescriptor[];
};
export type StreamProbeResult = {
  status: string;
  profile?: StreamServiceProfile | null;
  message?: string;
};

export const STREAM_MAPPING_LEGACY_REPAIR_REQUIRED_CODE = 40_901;

export const configStreamApi = {
  async getStreamMappings(): Promise<StreamMapping[]> {
    const res = await apiClient.get("/config/stream_mappings");
    return res.data.data;
  },
  async updateStreamMappings(mappings: StreamMapping[]): Promise<void> {
    const payload = { mappings } satisfies StreamMappingsUpdate;
    await apiClient.post("/config/stream_mappings", payload);
  },
  async getStreamServiceCatalog(): Promise<StreamServiceCatalog> {
    const res = await apiClient.get("/config/stream_service_catalog");
    return res.data.data;
  },
  async probeStreamMapping(mapping: StreamMapping): Promise<StreamProbeResult> {
    const res = await apiClient.post(
      `/config/stream_mappings/${mapping.protocol}/${mapping.listen_port}/probe`,
    );
    return res.data.data;
  },
  async confirmStreamServiceProfile(
    mapping: StreamMapping,
    serviceId: string,
  ): Promise<StreamServiceProfile> {
    const res = await apiClient.put(
      `/config/stream_mappings/${mapping.protocol}/${mapping.listen_port}/service_profile`,
      {
        expected_service_id: mapping.service_profile?.service_id ?? "",
        expected_target: mapping.target,
        service_id: serviceId,
      },
    );
    return res.data.data;
  },
  async clearStreamServiceProfile(mapping: StreamMapping): Promise<void> {
    await apiClient.put(
      `/config/stream_mappings/${mapping.protocol}/${mapping.listen_port}/service_profile`,
      {
        expected_service_id: mapping.service_profile?.service_id ?? "",
        expected_target: mapping.target,
        service_id: "",
      },
    );
  },
  async getStreamBypassPolicy(
    mapping: StreamMapping,
  ): Promise<StreamBypassPolicy> {
    const res = await apiClient.get(
      `/config/stream_mappings/${mapping.protocol}/${mapping.listen_port}/bypass_policy`,
    );
    return res.data.data;
  },
  async updateStreamBypassPolicy(
    mapping: StreamMapping,
    policy: StreamBypassPolicy,
  ): Promise<StreamBypassPolicy> {
    const res = await apiClient.put(
      `/config/stream_mappings/${mapping.protocol}/${mapping.listen_port}/bypass_policy`,
      {
        ...policy,
        expected_target: mapping.target,
        expected_use_auth: mapping.use_auth === true,
      },
    );
    return res.data.data;
  },
};
