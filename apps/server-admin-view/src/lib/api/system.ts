import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";

import type {
  DnsmasqInstallState,
  DnsmasqStatus,
  FnosCertificateSyncDetails,
  FnosCertificateSyncResponse,
  RunType,
} from "../../types";
import { apiClient } from "./client";

export type AccessEntryInfo =
  ApiContractComponents["schemas"]["AccessEntryData"];
export type RunModePromptPreferences =
  ApiContractComponents["schemas"]["RunModePromptPreferencesData"];
export type SystemClockIssue =
  ApiContractComponents["schemas"]["SystemClockIssueData"];
export type SystemClockIssueCode = SystemClockIssue["code"];
export type SystemClockStatus =
  ApiContractComponents["schemas"]["SystemClockStatusData"];

type AutoHttpsDetailsContract =
  ApiContractComponents["schemas"]["AutoHttpsDetailsData"];
type AutoHttpsUpdate = ApiContractComponents["schemas"]["AutoHttpsUpdateData"];
type FirewallAdditionalPortsContract =
  ApiContractComponents["schemas"]["FirewallAdditionalPortsData"];
type FirewallAdditionalPortsUpdate =
  ApiContractComponents["schemas"]["FirewallAdditionalPortsUpdateData"];
type FirewallResetBody =
  ApiContractComponents["schemas"]["FirewallResetBodyData"];
type FirewallResetResult =
  ApiContractComponents["schemas"]["FirewallResetData"];
type FirewallClearResult =
  ApiContractComponents["schemas"]["FirewallClearData"];
type ProtocolMappingFeatureContract =
  ApiContractComponents["schemas"]["ProtocolMappingFeatureData"];
type ProtocolMappingFeatureUpdate =
  ApiContractComponents["schemas"]["ProtocolMappingFeatureUpdateData"];
type RunModePromptPreferencesUpdate =
  ApiContractComponents["schemas"]["RunModePromptPreferencesUpdateData"];
type SmartConnectDetailsContract =
  ApiContractComponents["schemas"]["SmartConnectDetailsData"];
type SmartConnectUpdate =
  ApiContractComponents["schemas"]["SmartConnectUpdateData"];
type FnosShareBypassContract =
  ApiContractComponents["schemas"]["FnosShareBypassData"];
type FnosShareBypassUpdate =
  ApiContractComponents["schemas"]["FnosShareBypassUpdateData"];
type FnosPortIconHijackContract =
  ApiContractComponents["schemas"]["FnosPortIconHijackData"];
type FnosPortIconHijackUpdate =
  ApiContractComponents["schemas"]["FnosPortIconHijackUpdateData"];
type FnosConnectWafContract =
  ApiContractComponents["schemas"]["FnosConnectWafData"];
type FnosConnectWafUpdate =
  ApiContractComponents["schemas"]["FnosConnectWafUpdateData"];
type FnosNetworkTuningContract =
  ApiContractComponents["schemas"]["FnosNetworkTuningData"];
type FnosNetworkTuningUpdate =
  ApiContractComponents["schemas"]["FnosNetworkTuningUpdateData"];
type FnosCertificateSyncUpdate =
  ApiContractComponents["schemas"]["FnosCertificateSyncUpdateData"];
type FnosCertificateSyncBody =
  ApiContractComponents["schemas"]["FnosCertificateSyncBodyData"];
type SystemClockSyncResponse =
  ApiContractOperations["post_api_admin_system_clock_sync"]["responses"][200]["content"]["application/json"];
type SystemAssetMutationResponse =
  ApiContractComponents["schemas"]["SystemAssetMutationResponseData"];
type FrpAssetStatusResponse =
  ApiContractOperations["get_api_admin_system_frp_status"]["responses"][200]["content"]["application/json"];
export type CloudflaredAssetStatusResponse =
  ApiContractOperations["get_api_admin_system_cloudflared_status"]["responses"][200]["content"]["application/json"];
export type CloudflaredInstallationStatus =
  ApiContractComponents["schemas"]["CloudflaredAssetStatusData"]["installation_status"];

export const SystemAPI = {
  async getClockStatus(): Promise<SystemClockStatus> {
    const res = await apiClient.get("/system/clock/status");
    return res.data.data;
  },
  async refreshClockStatus(): Promise<SystemClockStatus> {
    const res = await apiClient.post("/system/clock/check");
    return res.data.data;
  },
  async syncClock(): Promise<SystemClockSyncResponse> {
    const res = await apiClient.post("/system/clock/sync");
    return res.data;
  },
  async getAccessEntry(): Promise<AccessEntryInfo> {
    const res = await apiClient.get("/system/access-entry");
    return res.data.data;
  },
  async resetFirewallByRunType(
    run_type: RunType,
  ): Promise<FirewallResetResult> {
    const body = { run_type } satisfies FirewallResetBody;
    const res = await apiClient.post("/firewall/reset", body);
    return res.data.data;
  },
  async clearFirewall(): Promise<FirewallClearResult> {
    const res = await apiClient.post("/firewall/clear");
    return res.data.data;
  },
  async getFirewallAdditionalPorts(): Promise<FirewallAdditionalPortsContract> {
    const res = await apiClient.get("/config/firewall_additional_ports");
    return res.data.data;
  },
  async updateFirewallAdditionalPorts(
    ports: number[],
  ): Promise<FirewallAdditionalPortsContract> {
    const body = { ports } satisfies FirewallAdditionalPortsUpdate;
    const res = await apiClient.post("/config/firewall_additional_ports", body);
    return res.data.data;
  },
  async getRunModePromptPreferences(): Promise<RunModePromptPreferences> {
    const res = await apiClient.get("/config/run_mode_prompt_preferences");
    return res.data.data;
  },
  async updateRunModePromptPreferences(
    payload: RunModePromptPreferencesUpdate,
  ): Promise<RunModePromptPreferences> {
    const res = await apiClient.post(
      "/config/run_mode_prompt_preferences",
      payload,
    );
    return res.data.data;
  },
  async getProtocolMappingFeatureConfig(): Promise<ProtocolMappingFeatureContract> {
    const res = await apiClient.get("/config/protocol_mapping_feature");
    return res.data.data;
  },
  async updateProtocolMappingFeatureConfig(
    payload: ProtocolMappingFeatureUpdate,
  ): Promise<ProtocolMappingFeatureContract> {
    const res = await apiClient.post(
      "/config/protocol_mapping_feature",
      payload,
    );
    return res.data.data;
  },
  async getAutoHttpsDetails(): Promise<AutoHttpsDetailsContract> {
    const res = await apiClient.get("/config/auto_https");
    return res.data.data;
  },
  async updateAutoHttps(
    payload: AutoHttpsUpdate,
  ): Promise<AutoHttpsDetailsContract> {
    const res = await apiClient.post("/config/auto_https", payload);
    return res.data.data;
  },
  async getSmartConnectDetails(
    signal?: AbortSignal,
  ): Promise<SmartConnectDetailsContract> {
    const res = await apiClient.get("/config/smart_connect/details", {
      signal,
    });
    return res.data.data;
  },
  async updateSmartConnect(
    payload: SmartConnectUpdate,
  ): Promise<SmartConnectDetailsContract> {
    const res = await apiClient.post("/config/smart_connect", payload);
    return res.data.data;
  },
  async getDnsmasqStatus(): Promise<DnsmasqStatus> {
    const res = await apiClient.get("/system/dnsmasq/status");
    return res.data.data;
  },
  async installDnsmasq(): Promise<DnsmasqInstallState> {
    const res = await apiClient.post("/system/dnsmasq/install");
    return res.data.data;
  },
  async getFnosShareBypassConfig(): Promise<FnosShareBypassContract> {
    const res = await apiClient.get("/config/fnos_share_bypass");
    return res.data.data;
  },
  async updateFnosShareBypassConfig(
    payload: FnosShareBypassUpdate,
  ): Promise<FnosShareBypassContract> {
    const res = await apiClient.post("/config/fnos_share_bypass", payload);
    return res.data.data;
  },
  async getFnosPortIconHijackConfig(): Promise<FnosPortIconHijackContract> {
    const res = await apiClient.get("/config/fnos_port_icon_hijack");
    return res.data.data;
  },
  async updateFnosPortIconHijackConfig(
    payload: FnosPortIconHijackUpdate,
  ): Promise<FnosPortIconHijackContract> {
    const res = await apiClient.post("/config/fnos_port_icon_hijack", payload);
    return res.data.data;
  },
  async getFnosConnectWafDetails(): Promise<FnosConnectWafContract> {
    const res = await apiClient.get("/config/fnos_connect_waf");
    return res.data.data;
  },
  async updateFnosConnectWafConfig(
    enabled: boolean,
  ): Promise<FnosConnectWafContract> {
    const body = { enabled } satisfies FnosConnectWafUpdate;
    const res = await apiClient.post("/config/fnos_connect_waf", body);
    return res.data.data;
  },
  async getFnosNetworkTuningStatus(): Promise<FnosNetworkTuningContract> {
    const res = await apiClient.get("/config/fnos_network_tuning");
    return res.data.data;
  },
  async updateFnosNetworkTuningConfig(
    payload: FnosNetworkTuningUpdate,
  ): Promise<FnosNetworkTuningContract> {
    const res = await apiClient.post("/config/fnos_network_tuning", payload);
    return res.data.data;
  },
  async getFnosCertificateSyncDetails(): Promise<FnosCertificateSyncDetails> {
    const res = await apiClient.get("/config/fnos_certificate_sync/details");
    return res.data.data;
  },
  async updateFnosCertificateSyncConfig(
    auto_sync_enabled: boolean,
  ): Promise<FnosCertificateSyncDetails> {
    const payload = {
      auto_sync_enabled,
    } satisfies FnosCertificateSyncUpdate;
    const res = await apiClient.post("/config/fnos_certificate_sync", payload);
    return res.data.data;
  },
  async syncFnosCertificates(
    target_ids: string[] = [],
  ): Promise<FnosCertificateSyncResponse> {
    const payload = { target_ids } satisfies FnosCertificateSyncBody;
    const res = await apiClient.post(
      "/config/fnos_certificate_sync/sync",
      payload,
    );
    return res.data.data;
  },
  async getFrpStatus(signal?: AbortSignal): Promise<FrpAssetStatusResponse> {
    const res = await apiClient.get("/system/frp/status", { signal });
    return res.data;
  },
  async startFrpDownload(): Promise<SystemAssetMutationResponse> {
    const res = await apiClient.post("/system/frp/download");
    return res.data;
  },
  async cancelFrpDownload(): Promise<SystemAssetMutationResponse> {
    const res = await apiClient.post("/system/frp/cancel");
    return res.data;
  },
  async deleteFrp(): Promise<SystemAssetMutationResponse> {
    const res = await apiClient.delete("/system/frp");
    return res.data;
  },
  async getCloudflaredStatus(
    signal?: AbortSignal,
  ): Promise<CloudflaredAssetStatusResponse> {
    const res = await apiClient.get("/system/cloudflared/status", { signal });
    return res.data;
  },
  async startCloudflaredDownload(): Promise<SystemAssetMutationResponse> {
    const res = await apiClient.post("/system/cloudflared/download");
    return res.data;
  },
  async cancelCloudflaredDownload(): Promise<SystemAssetMutationResponse> {
    const res = await apiClient.post("/system/cloudflared/cancel");
    return res.data;
  },
  async deleteCloudflared(): Promise<SystemAssetMutationResponse> {
    const res = await apiClient.delete("/system/cloudflared");
    return res.data;
  },
};
