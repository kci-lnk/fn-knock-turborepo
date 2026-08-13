import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";

import type {
  AppConfig,
  AdvancedAuthConfig,
  AuthAccount,
  AuthCredentialSettings,
  AuthLoginMode,
  AuthLoginModePreview,
  AuthLoginModeStatus,
  BackupDirectoryFilesPayload,
  AutomaticBackupDetails,
  AutomaticBackupFilesPayload,
  DashboardDisplayConfig,
  DockerAdminBootstrapState,
  FnKnockBackupExportToDirectoryResult,
  FnKnockBackupImportArchiveRequest,
  FnKnockBackupImportResult,
  GatewayHostResponseDetails,
  GatewayProxyHeadersDetails,
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
  ProxyMapping,
  WOLFeatureConfig,
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
import { apiClient } from "./client";
import {
  toHostMappingBasicAuthPayload,
  toHostMappingUpdatePayload,
} from "./host-mapping-payload";

const HOST_MAPPINGS_REVISION_HEADER = "x-host-mappings-revision";
const HOST_MAPPING_CATALOG_REVISION_HEADER = "x-host-mapping-catalog-revision";
export const STREAM_MAPPING_LEGACY_REPAIR_REQUIRED_CODE = 40_901;

type AuthCredentialSettingsUpdate =
  ApiContractComponents["schemas"]["AuthCredentialSettingsUpdateData"];
type DashboardDisplayUpdate =
  ApiContractComponents["schemas"]["DashboardDisplayUpdateData"];
type CaptchaSettings = ApiContractComponents["schemas"]["CaptchaSettingsData"];
type CaptchaSettingsUpdate =
  ApiContractComponents["schemas"]["CaptchaSettingsUpdateData"];
type RunTypeUpdate = ApiContractComponents["schemas"]["RunTypeUpdateData"];
type AutoManageFirewallUpdate =
  ApiContractComponents["schemas"]["AutoManageFirewallUpdateData"];
type AutoManageFirewallData =
  ApiContractComponents["schemas"]["AutoManageFirewallData"];
type TerminalFeature = ApiContractComponents["schemas"]["TerminalFeatureData"];
type TerminalFeatureUpdate =
  ApiContractComponents["schemas"]["TerminalFeatureUpdateData"];
type WelcomeGuide = ApiContractComponents["schemas"]["WelcomeGuideData"];
type AppearanceContract =
  ApiContractComponents["schemas"]["PanelAppearanceData"];
type DefaultRouteUpdate =
  ApiContractComponents["schemas"]["DefaultRouteUpdateData"];
type DefaultTunnelUpdate =
  ApiContractComponents["schemas"]["DefaultTunnelUpdateData"];
type MaintenanceClearBody =
  ApiContractComponents["schemas"]["MaintenanceClearBodyData"];
type MaintenanceClearResult =
  ApiContractComponents["schemas"]["MaintenanceClearData"];
type SyncRoutesResponse =
  ApiContractOperations["post_api_admin_sync_routes"]["responses"][200]["content"]["application/json"];
type ProxyProtocolForceContract =
  ApiContractComponents["schemas"]["ProxyProtocolForceData"];
type ProxyMappingsUpdate =
  ApiContractComponents["schemas"]["ProxyMappingsUpdateData"];
type StreamMappingsUpdate =
  ApiContractComponents["schemas"]["StreamMappingsUpdateData"];
type SubdomainModeUpdate =
  ApiContractComponents["schemas"]["SubdomainModeUpdateData"];
type SubdomainModeResponse =
  ApiContractComponents["schemas"]["SubdomainModeResponseData"];
type AdvancedAuthUpdate =
  ApiContractComponents["schemas"]["AdvancedAuthUpdateBodyData"];
type HostMappingMetadataBody =
  ApiContractComponents["schemas"]["HostMappingMetadataBodyData"];
type HostMappingBasicAuthProbeBody =
  ApiContractComponents["schemas"]["HostMappingBasicAuthProbeBodyData"];
type WOLFeatureConfigUpdate =
  ApiContractComponents["schemas"]["WolFeatureConfigUpdateData"];
type GatewayVisibilityUpdate =
  ApiContractComponents["schemas"]["GatewayVisibilityUpdateData"];
type GatewayProxyHeadersUpdate =
  ApiContractComponents["schemas"]["GatewayProxyHeadersUpdateData"];
type GatewayHostResponseUpdate =
  ApiContractComponents["schemas"]["GatewayHostResponseUpdateData"];
type GatewaySettingsUpdate =
  ApiContractComponents["schemas"]["GatewaySettingsUpdateData"];
type AuthAccountCreateRequest =
  ApiContractComponents["schemas"]["AuthAccountCreateBody"];
type AuthAccountPatchRequest =
  ApiContractComponents["schemas"]["AuthAccountPatchBody"];
type AuthAccountSetupRequest =
  ApiContractComponents["schemas"]["AuthAccountSetupBody"];
type OidcProviderCreateRequest =
  ApiContractComponents["schemas"]["OidcProviderCreateData"];
type OidcProviderUpdateRequest =
  ApiContractComponents["schemas"]["OidcProviderUpdateData"];
type LdapProviderCreateRequest =
  ApiContractComponents["schemas"]["LdapProviderCreateData"];
type LdapProviderUpdateRequest =
  ApiContractComponents["schemas"]["LdapProviderUpdateData"];
type LdapProviderTestRequest =
  ApiContractComponents["schemas"]["LdapProviderTestBodyData"];
type ExternalAuthConnectionTest =
  ApiContractComponents["schemas"]["ExternalAuthConnectionTestData"];
type ExternalAuthInvitationRequest =
  ApiContractComponents["schemas"]["ExternalAuthInvitationBodyData"];
type ExternalAuthInvitation =
  ApiContractComponents["schemas"]["ExternalAuthInvitationData"];
type PanelPasswordRequest =
  ApiContractComponents["schemas"]["PanelPasswordBodyData"];
type PanelLoginRequest = ApiContractComponents["schemas"]["PanelLoginBodyData"];
type SslSharedFileQuery =
  ApiContractOperations["get_api_admin_ssl_shared_files_content"]["parameters"]["query"];
type SslSharedFileContent =
  ApiContractComponents["schemas"]["SslSharedFileContentData"];
type SslCaStatus = ApiContractComponents["schemas"]["SslCaStatusData"];
type SslCaHostBody = ApiContractComponents["schemas"]["SslCaHostBodyData"];
type SslCaHostsDeleteBody =
  ApiContractComponents["schemas"]["SslCaHostsDeleteBodyData"];
type SslIssueResponse =
  ApiContractOperations["post_api_admin_ssl_ca_issue"]["responses"][200]["content"]["application/json"];
type SslActivateBody =
  ApiContractComponents["schemas"]["SslCertificateActivateBodyData"];
type SslDeploymentModeBody =
  ApiContractComponents["schemas"]["SslDeploymentModeBodyData"];

const hostMappingsRevisionFromHeaders = (
  headers: Record<string, unknown>,
): string | null => {
  const value = String(headers[HOST_MAPPINGS_REVISION_HEADER] ?? "").trim();
  return value || null;
};

export interface RevisionedConfig {
  config: AppConfig;
  hostMappingsRevision: string | null;
  hostMappingCatalogRevision: string | null;
}

export interface RevisionedHostMappings {
  mappings: HostMapping[];
  revision: string | null;
}

export interface RevisionedHostMappingCatalog {
  mappings: HostMapping[];
  groups: HostMappingGroup[];
  groupedView: boolean;
  revision: string | null;
  hostMappingsRevision: string | null;
}

export type AdvancedAuthDetails =
  ApiContractComponents["schemas"]["AdvancedAuthDetailsData"];

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
  TerminalFeatureConfig,
  TOTPCredential,
  TOTPCredentialImportSummary,
  TOTPSubdomainAccess,
  TOTPAccessScope,
  UrlMetadataPreview,
  WelcomeGuideStatus,
} from "../../types";

export type HostMappingBasicAuthProbeResult =
  ApiContractComponents["schemas"]["HostMappingBasicAuthProbeData"];

export const ConfigAPI = {
  async getDockerAdminBootstrap(): Promise<DockerAdminBootstrapState> {
    const res = await apiClient.get("/panel/bootstrap");
    return res.data.data;
  },
  async setDockerAdminPassword(
    password: string,
  ): Promise<DockerAdminBootstrapState> {
    const payload = { password } satisfies PanelPasswordRequest;
    const res = await apiClient.post("/panel/password", payload);
    return res.data.data;
  },
  async changeDockerAdminPassword(
    password: string,
  ): Promise<DockerAdminBootstrapState> {
    const payload = { password } satisfies PanelPasswordRequest;
    const res = await apiClient.post("/panel/password/change", payload);
    return res.data.data;
  },
  async loginDockerAdmin(
    password: string,
    rememberMe = false,
  ): Promise<DockerAdminBootstrapState> {
    const payload = { password, rememberMe } satisfies PanelLoginRequest;
    const res = await apiClient.post("/panel/login", payload);
    return res.data.data;
  },
  async logoutDockerAdmin(): Promise<DockerAdminBootstrapState> {
    const res = await apiClient.post("/panel/logout");
    return res.data.data;
  },
  async getConfig(): Promise<RevisionedConfig> {
    const res = await apiClient.get("/config");
    return {
      config: res.data.data,
      hostMappingsRevision: hostMappingsRevisionFromHeaders(res.headers),
      hostMappingCatalogRevision:
        String(
          res.headers[HOST_MAPPING_CATALOG_REVISION_HEADER] ?? "",
        ).trim() || null,
    };
  },
  async getLocaleConfig(): Promise<LocaleConfig> {
    const res = await apiClient.get("/config/locale");
    return res.data.data;
  },
  async getAppearanceConfig(): Promise<AppearanceContract> {
    const res = await apiClient.get("/config/appearance");
    return res.data.data;
  },
  async updateAppearanceConfig(
    payload: AppearanceContract,
  ): Promise<AppearanceContract> {
    const res = await apiClient.post("/config/appearance", payload);
    return res.data.data;
  },
  async updateLocaleConfig(payload: LocaleConfig): Promise<LocaleConfig> {
    const res = await apiClient.post("/config/locale", payload);
    return res.data.data;
  },
  async getWelcomeGuideStatus(): Promise<WelcomeGuide> {
    const res = await apiClient.get("/config/welcome_guide");
    return res.data.data;
  },
  async completeWelcomeGuide(): Promise<WelcomeGuide> {
    const res = await apiClient.post("/config/welcome_guide/complete");
    return res.data.data;
  },
  async updateRunType(payload: RunTypeUpdate): Promise<string | null> {
    const res = await apiClient.post("/config/run_type", payload);
    return typeof res.data.message === "string" ? res.data.message : null;
  },
  async updateAutoManageFirewall(
    payload: AutoManageFirewallUpdate,
  ): Promise<AutoManageFirewallData> {
    const res = await apiClient.post("/config/auto_manage_firewall", payload);
    return res.data.data;
  },
  async getTerminalFeature(): Promise<TerminalFeature> {
    const res = await apiClient.get("/config/terminal_feature");
    return res.data.data;
  },
  async getWOLFeature(): Promise<WOLFeatureConfig> {
    const res = await apiClient.get("/config/wol_feature");
    return res.data.data;
  },
  async getDashboardDisplayConfig(): Promise<DashboardDisplayConfig> {
    const res = await apiClient.get("/config/dashboard_display");
    return res.data.data;
  },
  async getAuthCredentialSettings(): Promise<AuthCredentialSettings> {
    const res = await apiClient.get("/config/auth_credential_settings");
    return res.data.data;
  },
  async updateDashboardDisplayConfig(
    payload: DashboardDisplayUpdate,
  ): Promise<DashboardDisplayConfig> {
    const res = await apiClient.post("/config/dashboard_display", payload);
    return res.data.data;
  },
  async updateAuthCredentialSettings(
    payload: AuthCredentialSettingsUpdate,
  ): Promise<AuthCredentialSettings> {
    const res = await apiClient.post(
      "/config/auth_credential_settings",
      payload,
    );
    return res.data.data;
  },
  async updateTerminalFeature(
    payload: TerminalFeatureUpdate,
  ): Promise<TerminalFeature> {
    const res = await apiClient.post("/config/terminal_feature", payload);
    return res.data.data;
  },
  async updateWOLFeature(
    payload: WOLFeatureConfigUpdate,
  ): Promise<WOLFeatureConfig> {
    const res = await apiClient.post("/config/wol_feature", payload);
    return res.data.data;
  },
  async updateDefaultTunnel(
    tunnel: DefaultTunnelUpdate["tunnel"],
  ): Promise<void> {
    const body = { tunnel } satisfies DefaultTunnelUpdate;
    await apiClient.post("/config/default_tunnel", body);
  },

  async updateProxyMappings(mappings: ProxyMapping[]): Promise<void> {
    const payload = { mappings } satisfies ProxyMappingsUpdate;
    await apiClient.post("/config/proxy_mappings", payload);
  },
  async getHostMappings(): Promise<RevisionedHostMappings> {
    const res = await apiClient.get("/config/host_mappings");
    return {
      mappings: res.data.data,
      revision: hostMappingsRevisionFromHeaders(res.headers),
    };
  },
  async getHostMappingCatalog(): Promise<RevisionedHostMappingCatalog> {
    const res = await apiClient.get("/config/host_mapping_catalog");
    const data = res.data.data;
    return {
      mappings: data.mappings,
      groups: data.groups,
      groupedView: data.grouped_view === true,
      revision:
        String(
          res.headers[HOST_MAPPING_CATALOG_REVISION_HEADER] ??
            data.revision ??
            "",
        ).trim() || null,
      hostMappingsRevision: hostMappingsRevisionFromHeaders(res.headers),
    };
  },
  async updateHostMappingCatalog(
    mappings: HostMapping[],
    groups: HostMappingGroup[],
    groupedView: boolean,
    revision: string | null,
    refreshedFaviconHosts: ReadonlySet<string> = new Set(),
    refreshedTitleHosts: ReadonlySet<string> = new Set(),
    previousHosts: ReadonlyMap<string, string> = new Map(),
  ): Promise<RevisionedHostMappingCatalog> {
    const res = await apiClient.post(
      "/config/host_mapping_catalog",
      {
        mappings: mappings.map((mapping) =>
          toHostMappingUpdatePayload(mapping, {
            includeFavicon: refreshedFaviconHosts.has(mapping.host),
            includeTitle: refreshedTitleHosts.has(mapping.host),
            previousHost: previousHosts.get(mapping.host),
          }),
        ),
        groups,
        grouped_view: groupedView,
        ...(revision ? { revision } : {}),
      },
      revision
        ? {
            headers: {
              [HOST_MAPPING_CATALOG_REVISION_HEADER]: revision,
            },
          }
        : undefined,
    );
    const data = res.data.data;
    return {
      mappings: data.mappings,
      groups: data.groups,
      groupedView: data.grouped_view === true,
      revision:
        String(
          res.headers[HOST_MAPPING_CATALOG_REVISION_HEADER] ??
            data.revision ??
            "",
        ).trim() || null,
      hostMappingsRevision: hostMappingsRevisionFromHeaders(res.headers),
    };
  },
  async updateHostMappings(
    mappings: HostMapping[],
    revision: string | null,
    refreshedFaviconHosts: ReadonlySet<string> = new Set(),
    refreshedTitleHosts: ReadonlySet<string> = new Set(),
  ): Promise<RevisionedHostMappings> {
    const res = await apiClient.post("/config/host_mappings", {
      mappings: mappings.map((mapping) =>
        toHostMappingUpdatePayload(mapping, {
          includeFavicon: refreshedFaviconHosts.has(mapping.host),
          includeTitle: refreshedTitleHosts.has(mapping.host),
        }),
      ),
      ...(revision ? { revision } : {}),
    });
    return {
      mappings: res.data.data,
      revision: hostMappingsRevisionFromHeaders(res.headers),
    };
  },
  async getAdvancedAuth(host: string): Promise<AdvancedAuthDetails> {
    const res = await apiClient.get(
      `/config/host_mappings/${encodeURIComponent(host)}/advanced_auth`,
    );
    return res.data.data;
  },
  async updateAdvancedAuth(
    host: string,
    revision: string | null,
    advancedAuth: AdvancedAuthConfig,
    acknowledgeBroadRules = false,
  ): Promise<AdvancedAuthDetails> {
    const payload = {
      revision: revision || undefined,
      advanced_auth: advancedAuth,
      acknowledge_broad_rules: acknowledgeBroadRules,
    } satisfies AdvancedAuthUpdate;
    const res = await apiClient.put(
      `/config/host_mappings/${encodeURIComponent(host)}/advanced_auth`,
      payload,
    );
    return res.data.data;
  },
  async refreshAllHostMappingTitles(): Promise<HostMappingRefreshSummary> {
    const res = await apiClient.post("/config/host_mappings/refresh_titles");
    return res.data.data;
  },
  async fetchHostMappingMetadata(
    target: string,
    basicAuth?: HostMappingBasicAuth | null,
  ): Promise<UrlMetadataPreview> {
    const payload = {
      target,
      ...(basicAuth
        ? { basic_auth: toHostMappingBasicAuthPayload(basicAuth) }
        : {}),
    } satisfies HostMappingMetadataBody;
    const res = await apiClient.post("/config/host_mappings/metadata", payload);
    return res.data.data;
  },
  async probeHostMappingBasicAuth(
    target: string,
  ): Promise<HostMappingBasicAuthProbeResult> {
    const payload = {
      target,
    } satisfies HostMappingBasicAuthProbeBody;
    const res = await apiClient.post(
      "/config/host_mappings/basic_auth_probe",
      payload,
    );
    return res.data.data;
  },
  async downloadHostMappingBookmarks(): Promise<Blob> {
    const res = await apiClient.get("/config/host_mappings/bookmarks/export", {
      responseType: "blob",
    });
    return res.data;
  },
  async getStreamMappings(): Promise<StreamMapping[]> {
    const res = await apiClient.get("/config/stream_mappings");
    return res.data.data;
  },
  async updateStreamMappings(mappings: StreamMapping[]): Promise<void> {
    const payload = { mappings } satisfies StreamMappingsUpdate;
    await apiClient.post("/config/stream_mappings", payload);
  },
  async getSubdomainMode(): Promise<SubdomainModeConfig> {
    const res = await apiClient.get("/config/subdomain_mode");
    return res.data.data;
  },
  async updateSubdomainMode(
    config: SubdomainModeUpdate,
  ): Promise<SubdomainModeResponse> {
    const res = await apiClient.post("/config/subdomain_mode", config);
    return res.data.data;
  },
  // SSL
  async getSSLStatus(): Promise<SSLStatus> {
    const res = await apiClient.get("/ssl/status");
    return res.data.data;
  },
  async getSSLSharedFiles(): Promise<SSLSharedFilesPayload> {
    const res = await apiClient.get("/ssl/shared-files");
    return res.data.data;
  },
  async readSSLSharedFile(path: string): Promise<SslSharedFileContent> {
    const params = { path } satisfies SslSharedFileQuery;
    const res = await apiClient.get("/ssl/shared-files/content", {
      params,
    });
    return res.data.data;
  },
  // CA
  async getCAStatus(): Promise<SslCaStatus> {
    const res = await apiClient.get("/ssl/ca/status");
    return res.data.data;
  },
  async initCA(): Promise<void> {
    await apiClient.post("/ssl/ca/init");
  },
  async clearCA(): Promise<void> {
    await apiClient.delete("/ssl/ca");
  },
  async downloadCACert(): Promise<Blob> {
    const res = await apiClient.get("/ssl/ca/cert.pem", {
      responseType: "blob",
    });
    return res.data;
  },
  async getCAHosts(): Promise<string[]> {
    const res = await apiClient.get("/ssl/ca/hosts");
    return res.data.data || [];
  },
  async addCAHost(value: string): Promise<string[]> {
    const payload = { value } satisfies SslCaHostBody;
    const res = await apiClient.post("/ssl/ca/hosts", payload);
    return res.data.data || [];
  },
  async removeCAHost(value: string): Promise<string[]> {
    const data = { value } satisfies SslCaHostsDeleteBody;
    const res = await apiClient.delete("/ssl/ca/hosts", { data });
    return res.data.data || [];
  },
  async clearCAHosts(): Promise<void> {
    const data = { all: true } satisfies SslCaHostsDeleteBody;
    await apiClient.delete("/ssl/ca/hosts", { data });
  },
  async issueAndInstall(): Promise<SslIssueResponse> {
    const res = await apiClient.post("/ssl/ca/issue");
    return res.data;
  },
  async downloadServerCert(): Promise<Blob> {
    const res = await apiClient.get("/ssl/ca/server-cert.zip", {
      responseType: "blob",
    });
    return res.data;
  },
  async setSSL(ssl: SSLConfig): Promise<void> {
    await apiClient.post("/ssl/certificates", ssl);
  },
  async deleteSSL(): Promise<void> {
    await apiClient.delete("/ssl");
  },
  async updateSSLDeploymentMode(
    deployment_mode: SslDeploymentModeBody["deployment_mode"],
  ): Promise<SSLStatus> {
    const payload = { deployment_mode } satisfies SslDeploymentModeBody;
    const res = await apiClient.post("/ssl/deployment-mode", payload);
    return res.data.data;
  },
  async activateSSLCertificate(id: string): Promise<void> {
    const payload = { id } satisfies SslActivateBody;
    await apiClient.post("/ssl/activate", payload);
  },
  async deleteSSLCertificate(id: string): Promise<void> {
    await apiClient.delete(`/ssl/certificates/${encodeURIComponent(id)}`);
  },
  async clearSSLCertificateLibrary(): Promise<void> {
    await apiClient.delete("/ssl/certificates");
  },
  async updateDefaultRoute(path: string): Promise<void> {
    const body = { path } satisfies DefaultRouteUpdate;
    await apiClient.post("/config/default_route", body);
  },
  async getGatewaySettings(): Promise<GatewaySettings> {
    const res = await apiClient.get("/config/gateway");
    return res.data.data;
  },
  async updateGatewaySettings(
    payload: GatewaySettingsUpdate,
  ): Promise<GatewaySettings> {
    const res = await apiClient.post("/config/gateway", payload);
    return res.data.data;
  },
  async getGatewayVisibility(): Promise<GatewayVisibilityDetails> {
    const res = await apiClient.get("/config/gateway/visibility");
    return res.data.data;
  },
  async updateGatewayVisibility(
    payload: GatewayVisibilityUpdate,
  ): Promise<GatewayVisibilityDetails> {
    const res = await apiClient.post("/config/gateway/visibility", payload);
    return res.data.data;
  },
  async getGatewayProxyHeaders(): Promise<GatewayProxyHeadersDetails> {
    const res = await apiClient.get("/config/gateway/proxy-headers");
    return res.data.data;
  },
  async updateGatewayProxyHeaders(
    payload: GatewayProxyHeadersUpdate,
  ): Promise<GatewayProxyHeadersDetails> {
    const res = await apiClient.post("/config/gateway/proxy-headers", payload);
    return res.data.data;
  },
  async getGatewayHostResponse(): Promise<GatewayHostResponseDetails> {
    const res = await apiClient.get("/config/gateway/host-response");
    return res.data.data;
  },
  async updateGatewayHostResponse(
    payload: GatewayHostResponseUpdate,
  ): Promise<GatewayHostResponseDetails> {
    const res = await apiClient.post("/config/gateway/host-response", payload);
    return res.data.data;
  },
  async getProxyProtocolForce(): Promise<ProxyProtocolForceContract> {
    const res = await apiClient.get("/config/proxy_protocol_force");
    return res.data.data;
  },
  async setProxyProtocolForce(
    proxy_protocol_force: boolean,
  ): Promise<ProxyProtocolForceContract> {
    const body = {
      proxy_protocol_force,
    } satisfies ProxyProtocolForceContract;
    const res = await apiClient.post("/config/proxy_protocol_force", body);
    return res.data.data;
  },
  // TOTP
  async getTOTPStatus(): Promise<{
    bound: boolean;
    credentials: TOTPCredential[];
  }> {
    const res = await apiClient.get("/totp/status");
    return res.data.data;
  },
  async getAuthLoginMode(): Promise<AuthLoginModeStatus> {
    const res = await apiClient.get("/auth/mode");
    return res.data.data;
  },
  async previewAuthLoginMode(
    mode: AuthLoginMode,
  ): Promise<AuthLoginModePreview> {
    const res = await apiClient.post("/auth/mode/preview", { mode });
    return res.data.data;
  },
  async switchAuthLoginMode(mode: AuthLoginMode): Promise<AuthLoginModeStatus> {
    const res = await apiClient.post("/auth/mode/switch", { mode });
    return res.data.data;
  },
  async getAuthAccounts(): Promise<AuthAccount[]> {
    const res = await apiClient.get("/auth/accounts");
    return res.data.data.accounts || [];
  },
  async createAuthAccount(
    payload: AuthAccountCreateRequest,
  ): Promise<AuthAccount> {
    const res = await apiClient.post("/auth/accounts", payload);
    return res.data.data;
  },
  async updateAuthAccount(
    id: string,
    payload: AuthAccountPatchRequest,
  ): Promise<AuthAccount> {
    const res = await apiClient.patch(
      `/auth/accounts/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data.data;
  },
  async deleteAuthAccount(id: string): Promise<void> {
    await apiClient.delete(`/auth/accounts/${encodeURIComponent(id)}`);
  },
  async setAuthAccountPassword(
    id: string,
    password: string,
  ): Promise<AuthAccount> {
    const res = await apiClient.post(
      `/auth/accounts/${encodeURIComponent(id)}/password`,
      { password },
    );
    return res.data.data;
  },
  async setupAuthAccount(
    id: string,
    payload: AuthAccountSetupRequest,
  ): Promise<AuthAccount> {
    const res = await apiClient.post(
      `/auth/accounts/${encodeURIComponent(id)}/setup`,
      payload,
    );
    return res.data.data;
  },
  async setupAuthAccountTOTP(
    id: string,
  ): Promise<{ secret: string; uri: string }> {
    const res = await apiClient.post(
      `/auth/accounts/${encodeURIComponent(id)}/totp/setup`,
    );
    return res.data.data;
  },
  async bindAuthAccountTOTP(
    id: string,
    secret: string,
    token: string,
  ): Promise<AuthAccount> {
    const res = await apiClient.post(
      `/auth/accounts/${encodeURIComponent(id)}/totp/bind`,
      { secret, token },
    );
    return res.data.data;
  },
  async updateAuthAccountAccessScopes(
    id: string,
    accessScopes: TOTPAccessScope[],
  ): Promise<AuthAccount> {
    const res = await apiClient.patch(
      `/auth/accounts/${encodeURIComponent(id)}/access-scopes`,
      {
        access_scopes: accessScopes,
      },
    );
    return res.data.data;
  },
  async updateAuthAccountSubdomainAccess(
    id: string,
    subdomainAccess: TOTPSubdomainAccess,
  ): Promise<AuthAccount> {
    const res = await apiClient.patch(
      `/auth/accounts/${encodeURIComponent(id)}/subdomain-access`,
      {
        subdomain_access: subdomainAccess,
      },
    );
    return res.data.data;
  },
  async setupTOTP(): Promise<{ secret: string; uri: string }> {
    const res = await apiClient.post("/totp/setup");
    return res.data.data;
  },
  async bindTOTP(
    secret: string,
    token: string,
    comment?: string,
  ): Promise<{ success: boolean; message?: string }> {
    const res = await apiClient.post("/totp/bind", { secret, token, comment });
    return res.data;
  },
  async downloadTOTPCredentials(): Promise<Blob> {
    const res = await apiClient.get("/totp/credentials/export", {
      responseType: "blob",
    });
    return res.data;
  },
  async importTOTPCredentials(
    payload: unknown,
  ): Promise<TOTPCredentialImportSummary> {
    const res = await apiClient.post("/totp/credentials/import", { payload });
    return res.data.data;
  },
  async deleteTOTP(id: string): Promise<void> {
    await apiClient.delete(`/totp/${encodeURIComponent(id)}`);
  },
  async updateTOTPComment(id: string, comment: string): Promise<void> {
    await apiClient.patch(`/totp/${encodeURIComponent(id)}/comment`, {
      comment,
    });
  },
  async updateTOTPAccessScopes(
    id: string,
    accessScopes: TOTPAccessScope[],
  ): Promise<TOTPCredential> {
    const res = await apiClient.patch(
      `/totp/${encodeURIComponent(id)}/access-scopes`,
      {
        access_scopes: accessScopes,
      },
    );
    return res.data.data;
  },
  async updateTOTPSubdomainAccess(
    id: string,
    subdomainAccess: TOTPSubdomainAccess,
  ): Promise<TOTPCredential> {
    const res = await apiClient.patch(
      `/totp/${encodeURIComponent(id)}/subdomain-access`,
      {
        subdomain_access: subdomainAccess,
      },
    );
    return res.data.data;
  },
  async getPasskeys(totpId: string): Promise<PasskeyCredential[]> {
    const res = await apiClient.get(
      `/totp/${encodeURIComponent(totpId)}/passkeys`,
    );
    return res.data.data;
  },
  async deletePasskey(id: string): Promise<void> {
    await apiClient.delete(`/passkeys/${encodeURIComponent(id)}`);
  },
  async getOIDCProviderCatalog(): Promise<OIDCProviderCatalogItem[]> {
    const res = await apiClient.get("/auth/oidc/catalog");
    return res.data.data.providers;
  },
  async getOIDCProviders(): Promise<OIDCProviderView[]> {
    const res = await apiClient.get("/auth/oidc/providers");
    return res.data.data.providers;
  },
  async createOIDCProvider(
    payload: OidcProviderCreateRequest,
  ): Promise<OIDCProviderView> {
    const res = await apiClient.post("/auth/oidc/providers", payload);
    return res.data.data;
  },
  async updateOIDCProvider(
    id: string,
    payload: OidcProviderUpdateRequest,
  ): Promise<OIDCProviderView> {
    const res = await apiClient.patch(
      `/auth/oidc/providers/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data.data;
  },
  async deleteOIDCProvider(id: string): Promise<void> {
    await apiClient.delete(`/auth/oidc/providers/${encodeURIComponent(id)}`);
  },
  async testOIDCProvider(id: string): Promise<ExternalAuthConnectionTest> {
    const res = await apiClient.post(
      `/auth/oidc/providers/${encodeURIComponent(id)}/test`,
    );
    return res.data;
  },
  async getOIDCBindings(totpId: string): Promise<OIDCBinding[]> {
    const res = await apiClient.get(
      `/auth/oidc/totp/${encodeURIComponent(totpId)}/bindings`,
    );
    return res.data.data.bindings;
  },
  async deleteOIDCBinding(id: string): Promise<void> {
    await apiClient.delete(`/auth/oidc/bindings/${encodeURIComponent(id)}`);
  },
  async createOIDCInvite(
    payload: ExternalAuthInvitationRequest,
  ): Promise<ExternalAuthInvitation> {
    const res = await apiClient.post("/auth/oidc/invitations", payload);
    return res.data.data;
  },
  async getLdapProviderCatalog(): Promise<LdapProviderCatalogItem[]> {
    const res = await apiClient.get("/auth/ldap/catalog");
    return res.data.data.providers;
  },
  async getLdapProviders(): Promise<LdapProviderView[]> {
    const res = await apiClient.get("/auth/ldap/providers");
    return res.data.data.providers;
  },
  async createLdapProvider(
    payload: LdapProviderCreateRequest,
  ): Promise<LdapProviderView> {
    const res = await apiClient.post("/auth/ldap/providers", payload);
    return res.data.data;
  },
  async updateLdapProvider(
    id: string,
    payload: LdapProviderUpdateRequest,
  ): Promise<LdapProviderView> {
    const res = await apiClient.patch(
      `/auth/ldap/providers/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data.data;
  },
  async deleteLdapProvider(id: string): Promise<void> {
    await apiClient.delete(`/auth/ldap/providers/${encodeURIComponent(id)}`);
  },
  async testLdapProvider(
    id: string,
    credentials?: LdapProviderTestRequest,
  ): Promise<ExternalAuthConnectionTest> {
    const res = await apiClient.post(
      `/auth/ldap/providers/${encodeURIComponent(id)}/test`,
      credentials ?? {},
    );
    return res.data;
  },
  async getLdapBindings(totpId: string): Promise<LdapBinding[]> {
    const res = await apiClient.get(
      `/auth/ldap/totp/${encodeURIComponent(totpId)}/bindings`,
    );
    return res.data.data.bindings;
  },
  async deleteLdapBinding(id: string): Promise<void> {
    await apiClient.delete(`/auth/ldap/bindings/${encodeURIComponent(id)}`);
  },
  async createLdapInvite(
    payload: ExternalAuthInvitationRequest,
  ): Promise<ExternalAuthInvitation> {
    const res = await apiClient.post("/auth/ldap/invitations", payload);
    return res.data.data;
  },
  // Sync routes.
  async syncRoutes(): Promise<SyncRoutesResponse> {
    const res = await apiClient.post("/sync-routes");
    return res.data;
  },
};

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
  async getStatus(): Promise<UpdateStatusPayload> {
    const res = await apiClient.get("/update/status");
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
