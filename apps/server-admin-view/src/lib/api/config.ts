import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";
import type {
  AutomaticBackupDetails,
  AutomaticBackupFilesPayload,
  BackupDirectoryFilesPayload,
  FnKnockBackupExportToDirectoryResult,
  FnKnockBackupImportArchiveRequest,
  FnKnockBackupImportResult,
} from "../../types";
import { apiClient } from "./client";
import { configAuthApi } from "./config-auth-api";
import { configCoreApi } from "./config-core-api";
import { configHostMappingStaticApi } from "./config-host-mapping-static-api";
import { configProxyApi } from "./config-proxy-api";
import { configSslLanApi } from "./config-ssl-lan-api";
import {
  configStreamApi,
  STREAM_MAPPING_LEGACY_REPAIR_REQUIRED_CODE,
} from "./config-stream-api";

type CaptchaSettings = ApiContractComponents["schemas"]["CaptchaSettingsData"];
type CaptchaSettingsUpdate =
  ApiContractComponents["schemas"]["CaptchaSettingsUpdateData"];
type MaintenanceClearBody =
  ApiContractComponents["schemas"]["MaintenanceClearBodyData"];
type MaintenanceClearResult =
  ApiContractComponents["schemas"]["MaintenanceClearData"];

export const ConfigAPI = {
  ...configCoreApi,
  ...configHostMappingStaticApi,
  ...configProxyApi,
  ...configSslLanApi,
  ...configStreamApi,
  ...configAuthApi,
};

export { STREAM_MAPPING_LEGACY_REPAIR_REQUIRED_CODE };
export type {
  AdvancedAuthDetails,
  HostMappingBasicAuthProbeResult,
} from "./config-proxy-api";
export type {
  StreamBypassPolicy,
  StreamProbeResult,
  StreamServiceCatalog,
  StreamServiceDescriptor,
  StreamServiceProfile,
} from "./config-stream-api";
export type {
  StaticPathProbeErrorCode,
  StaticPathProbeResult,
  StaticPathProbeTargetType,
} from "./config-host-mapping-static-api";
export type {
  RevisionedConfig,
  RevisionedHostMappingCatalog,
  RevisionedHostMappings,
} from "./config-revisions";

export type {
  AppConfig,
  AppearanceConfig,
  AuthAccount,
  AuthCredentialSettings,
  AuthLoginMode,
  AuthLoginModePreview,
  AuthLoginModeStatus,
  AutoHttpsConfig,
  AutoHttpsDetails,
  BackupDirectoryFilesPayload,
  DashboardDisplayConfig,
  DnsmasqInstallState,
  DnsmasqStatus,
  DockerAdminBootstrapState,
  FnKnockBackupExportToDirectoryResult,
  FnKnockBackupImportArchiveRequest,
  FnKnockBackupImportResult,
  FnosNetworkTuningStatus,
  FnosNetworkTuningUpdatePayload,
  FnosConnectWafDetails,
  FnosPortIconHijackConfig,
  FnosShareBypassConfig,
  GatewayHostResponseDetails,
  GatewayLoggingConfig,
  GatewayPortalConfig,
  GatewayProxyHeadersDetails,
  GatewayProxyProtocolConfig,
  GatewaySettings,
  GatewayVisibilityDetails,
  HostMapping,
  HostMappingBasicAuth,
  HostMappingGroup,
  HostMappingRefreshSummary,
  LocaleConfig,
  LdapBinding,
  LdapProviderCatalogItem,
  LdapProviderView,
  OIDCBinding,
  OIDCProviderCatalogItem,
  OIDCProviderView,
  PasskeyCredential,
  ProtocolMappingFeatureConfig,
  ProxyMapping,
  ProxyProtocolForce,
  ReverseProxySubmode,
  RuntimeCapabilities,
  RuntimeProfile,
  SharedDataFileEntry,
  SmartConnectConfig,
  SmartConnectDetails,
  SSLConfig,
  SSLSharedFilesPayload,
  SSLStatus,
  StreamMapping,
  SubdomainModeConfig,
  TOTPCredential,
  TOTPCredentialImportSummary,
  TOTPSubdomainAccess,
  TOTPAccessScope,
  UrlMetadataPreview,
} from "../../types";

export const MaintenanceAPI = {
  async getAutomaticBackupDetails(): Promise<AutomaticBackupDetails> {
    const res = await apiClient.get("/maintenance/backup/automatic");
    return res.data.data;
  },
  async updateAutomaticBackupConfig(
    payload: ApiContractComponents["schemas"]["UpdateAutomaticBackupBody"],
  ): Promise<AutomaticBackupDetails> {
    const res = await apiClient.put("/maintenance/backup/automatic", payload);
    return res.data.data;
  },
  async getAutomaticBackupFiles(): Promise<AutomaticBackupFilesPayload> {
    const res = await apiClient.get("/maintenance/backup/automatic/files");
    return res.data.data;
  },
  async downloadBackup(): Promise<Blob> {
    const res = await apiClient.get("/maintenance/backup/export", {
      responseType: "blob",
    });
    return res.data;
  },
  async getBackupDirectoryFiles(): Promise<BackupDirectoryFilesPayload> {
    const res = await apiClient.get("/maintenance/backup/files");
    return res.data.data;
  },
  async exportBackupToFnos(): Promise<FnKnockBackupExportToDirectoryResult> {
    const res = await apiClient.post("/maintenance/backup/export/fnos");
    return res.data.data;
  },
  async importBackup(
    payload: FnKnockBackupImportArchiveRequest,
  ): Promise<FnKnockBackupImportResult> {
    const res = await apiClient.post("/maintenance/backup/import", payload);
    return res.data.data;
  },
  async importBackupFromFnos(path: string): Promise<FnKnockBackupImportResult> {
    const res = await apiClient.post("/maintenance/backup/import/fnos", {
      path,
    });
    return res.data.data;
  },
  async importBackupFromAutomatic(
    path: string,
  ): Promise<FnKnockBackupImportResult> {
    const res = await apiClient.post("/maintenance/backup/import/automatic", {
      path,
    });
    return res.data.data;
  },
  async clearAllData(confirmation: string): Promise<MaintenanceClearResult> {
    const body = {
      confirmation,
    } satisfies MaintenanceClearBody;
    const res = await apiClient.post("/maintenance/data/clear", body);
    return res.data.data;
  },
};

export {
  SystemAPI,
  type AccessEntryInfo,
  type RunModePromptPreferences,
  type SystemClockIssue,
  type SystemClockIssueCode,
  type SystemClockStatus,
} from "./system";

export type UpdateStatusPayload =
  ApiContractComponents["schemas"]["UpdateStatusData"];
export type UpdateDownloadStatus = UpdateStatusPayload["download"]["status"];
export type UpdateLatestPayload =
  ApiContractComponents["schemas"]["UpdateLatestData"];
export type UpdateConfirmPayload =
  ApiContractComponents["schemas"]["UpdateConfirmData"];

type UpdateStatusResponse =
  ApiContractOperations["get_api_admin_update_status"]["responses"][200]["content"]["application/json"];
type UpdateCheckResponse =
  ApiContractOperations["post_api_admin_update_check"]["responses"][200]["content"]["application/json"];
type UpdateCheckAndDownloadResponse =
  ApiContractOperations["post_api_admin_update_check_and_download"]["responses"][200]["content"]["application/json"];
type UpdateDownloadResponse =
  ApiContractOperations["post_api_admin_update_download"]["responses"][200]["content"]["application/json"];
type UpdateInstallResponse =
  ApiContractOperations["post_api_admin_update_install"]["responses"][200]["content"]["application/json"];
type UpdateConfirmResponse =
  ApiContractOperations["get_api_admin_update_confirm"]["responses"][200]["content"]["application/json"];

export const CaptchaAPI = {
  async getSettings(): Promise<CaptchaSettings> {
    const res = await apiClient.get("/config/captcha");
    return res.data.data;
  },
  async updateSettings(
    payload: CaptchaSettingsUpdate,
  ): Promise<CaptchaSettings> {
    const res = await apiClient.post("/config/captcha", payload);
    return res.data.data;
  },
};

export type IpLocationApiConfig =
  ApiContractComponents["schemas"]["IpLocationApiConfigData"];
export type IpLocationApiMode = IpLocationApiConfig["ip_lookup_mode"];
type IpLocationTestUrlBody =
  ApiContractComponents["schemas"]["IpLocationTestUrlBodyData"];
type IpLocationTestResponse =
  ApiContractOperations["post_api_admin_config_ip_location_api_test_ip_lookup"]["responses"][200]["content"]["application/json"];
type CidrTestResponse =
  ApiContractOperations["post_api_admin_config_ip_location_api_test_cidr"]["responses"][200]["content"]["application/json"];

export const IpLocationSettingsAPI = {
  async getSettings(): Promise<IpLocationApiConfig> {
    const res = await apiClient.get("/config/ip_location_api");
    return res.data.data;
  },
  async updateSettings(
    payload: IpLocationApiConfig,
  ): Promise<IpLocationApiConfig> {
    const res = await apiClient.post("/config/ip_location_api", payload);
    return res.data.data;
  },
  async testIpLookup(url: string): Promise<IpLocationTestResponse> {
    const body = { url } satisfies IpLocationTestUrlBody;
    const res = await apiClient.post(
      "/config/ip_location_api/test-ip-lookup",
      body,
    );
    return res.data;
  },
  async testCidr(url: string): Promise<CidrTestResponse> {
    const body = { url } satisfies IpLocationTestUrlBody;
    const res = await apiClient.post("/config/ip_location_api/test-cidr", body);
    return res.data;
  },
};

export const UpdateAPI = {
  async getStatus(cacheBust = false): Promise<UpdateStatusPayload> {
    const res = await apiClient.get("/update/status", {
      params: cacheBust ? { _fn_knock_update_probe: Date.now() } : undefined,
    });
    const payload = res.data as UpdateStatusResponse;
    return payload.data;
  },
  async checkNow(): Promise<UpdateStatusPayload> {
    const res = await apiClient.post("/update/check");
    const payload = res.data as UpdateCheckResponse;
    return payload.data;
  },
  async checkAndDownload(): Promise<UpdateCheckAndDownloadResponse> {
    const res = await apiClient.post("/update/check-and-download");
    return res.data;
  },
  async startDownload(): Promise<UpdateDownloadResponse> {
    const res = await apiClient.post("/update/download");
    return res.data;
  },
  async startInstall(): Promise<UpdateInstallResponse> {
    const res = await apiClient.post("/update/install");
    return res.data;
  },
  async consumeConfirm(): Promise<UpdateConfirmPayload | null> {
    const res = await apiClient.get("/update/confirm");
    const payload = res.data as UpdateConfirmResponse;
    return payload.data;
  },
};

export type BackoffItem = ApiContractComponents["schemas"]["LoginBackoffData"];

type BackoffListResponse =
  ApiContractOperations["get_api_admin_backoff_list"]["responses"][200]["content"]["application/json"];
type BackoffStatusOperation =
  ApiContractOperations["get_api_admin_backoff_status"];
type BackoffStatusQuery = BackoffStatusOperation["parameters"]["query"];
type BackoffStatusResponse =
  BackoffStatusOperation["responses"][200]["content"]["application/json"];
type BackoffResetOperation =
  ApiContractOperations["post_api_admin_backoff_reset"];
type BackoffResetBody =
  BackoffResetOperation["requestBody"]["content"]["application/json"];

export const BackoffAPI = {
  async list(): Promise<BackoffItem[]> {
    const res = await apiClient.get("/backoff/list");
    const payload = res.data as BackoffListResponse;
    return payload.data;
  },
  async status(ip: string): Promise<BackoffItem> {
    const params = { ip } satisfies BackoffStatusQuery;
    const res = await apiClient.get("/backoff/status", { params });
    const payload = res.data as BackoffStatusResponse;
    return payload.data;
  },
  async reset(ip: string): Promise<void> {
    const body = { ip } satisfies BackoffResetBody;
    await apiClient.post("/backoff/reset", body);
  },
};
