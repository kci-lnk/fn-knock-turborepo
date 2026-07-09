import type { CaptchaSettings } from "@frontend-core/captcha/types";

import type {
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
  FnosPortIconHijackConfig,
  FnosShareBypassConfig,
  GatewayHostResponseDetails,
  GatewayPortalConfig,
  GatewayProxyHeadersDetails,
  GatewaySettings,
  GatewayVisibilityDetails,
  HostMapping,
  HostMappingBasicAuth,
  HostMappingRefreshSummary,
  LocaleConfig,
  OIDCBinding,
  OIDCProviderCatalogItem,
  OIDCProviderView,
  PasskeyCredential,
  ProtocolMappingFeatureConfig,
  ProxyMapping,
  ProxyProtocolForce,
  ReverseProxySubmode,
  RunType,
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
import { apiClient } from "./client";
import {
  toHostMappingBasicAuthPayload,
  toHostMappingUpdatePayload,
} from "./host-mapping-payload";

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
  HostMappingRefreshSummary,
  LocaleConfig,
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

export interface HostMappingBasicAuthProbeResult {
  requiresBasicAuth: boolean;
  httpStatus: number | null;
  error?: string;
}

export const ConfigAPI = {
  async getDockerAdminBootstrap(): Promise<DockerAdminBootstrapState> {
    const res = await apiClient.get("/panel/bootstrap");
    return res.data.data;
  },
  async setDockerAdminPassword(
    password: string,
  ): Promise<DockerAdminBootstrapState> {
    const res = await apiClient.post("/panel/password", { password });
    return res.data.data;
  },
  async changeDockerAdminPassword(
    password: string,
  ): Promise<DockerAdminBootstrapState> {
    const res = await apiClient.post("/panel/password/change", { password });
    return res.data.data;
  },
  async loginDockerAdmin(
    password: string,
    rememberMe = false,
  ): Promise<DockerAdminBootstrapState> {
    const res = await apiClient.post("/panel/login", { password, rememberMe });
    return res.data.data;
  },
  async logoutDockerAdmin(): Promise<DockerAdminBootstrapState> {
    const res = await apiClient.post("/panel/logout");
    return res.data.data;
  },
  async getConfig(): Promise<AppConfig> {
    const res = await apiClient.get("/config");
    return res.data.data;
  },
  async getLocaleConfig(): Promise<LocaleConfig> {
    const res = await apiClient.get("/config/locale");
    return res.data.data;
  },
  async getAppearanceConfig(): Promise<AppearanceConfig> {
    const res = await apiClient.get("/config/appearance");
    return res.data.data;
  },
  async updateAppearanceConfig(
    payload: Partial<AppearanceConfig>,
  ): Promise<AppearanceConfig> {
    const res = await apiClient.post("/config/appearance", payload);
    return res.data.data;
  },
  async updateLocaleConfig(payload: LocaleConfig): Promise<LocaleConfig> {
    const res = await apiClient.post("/config/locale", payload);
    return res.data.data;
  },
  async getWelcomeGuideStatus(): Promise<WelcomeGuideStatus> {
    const res = await apiClient.get("/config/welcome_guide");
    return res.data.data;
  },
  async completeWelcomeGuide(): Promise<WelcomeGuideStatus> {
    const res = await apiClient.post("/config/welcome_guide/complete");
    return res.data.data;
  },
  async updateRunType(payload: {
    run_type: RunType;
    reverse_proxy_submode?: ReverseProxySubmode;
  }): Promise<void> {
    await apiClient.post("/config/run_type", payload);
  },
  async updateAutoManageFirewall(payload: {
    auto_manage_firewall: boolean;
  }): Promise<{
    auto_manage_firewall: boolean;
  }> {
    const res = await apiClient.post("/config/auto_manage_firewall", payload);
    return res.data.data;
  },
  async getTerminalFeature(): Promise<TerminalFeatureConfig> {
    const res = await apiClient.get("/config/terminal_feature");
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
    payload: Partial<DashboardDisplayConfig>,
  ): Promise<DashboardDisplayConfig> {
    const res = await apiClient.post("/config/dashboard_display", payload);
    return res.data.data;
  },
  async updateAuthCredentialSettings(
    payload: Partial<AuthCredentialSettings>,
  ): Promise<AuthCredentialSettings> {
    const res = await apiClient.post(
      "/config/auth_credential_settings",
      payload,
    );
    return res.data.data;
  },
  async updateTerminalFeature(
    payload: Partial<TerminalFeatureConfig>,
  ): Promise<TerminalFeatureConfig> {
    const res = await apiClient.post("/config/terminal_feature", payload);
    return res.data.data;
  },
  async updateDefaultTunnel(tunnel: "frp" | "cloudflared"): Promise<void> {
    await apiClient.post("/config/default_tunnel", { tunnel });
  },

  async updateProxyMappings(mappings: ProxyMapping[]): Promise<void> {
    await apiClient.post("/config/proxy_mappings", { mappings });
  },
  async getHostMappings(): Promise<HostMapping[]> {
    const res = await apiClient.get("/config/host_mappings");
    return res.data.data;
  },
  async updateHostMappings(mappings: HostMapping[]): Promise<HostMapping[]> {
    const res = await apiClient.post("/config/host_mappings", {
      mappings: mappings.map(toHostMappingUpdatePayload),
    });
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
    const res = await apiClient.post("/config/host_mappings/metadata", {
      target,
      ...(basicAuth
        ? { basic_auth: toHostMappingBasicAuthPayload(basicAuth) }
        : {}),
    });
    return res.data.data;
  },
  async probeHostMappingBasicAuth(
    target: string,
  ): Promise<HostMappingBasicAuthProbeResult> {
    const res = await apiClient.post("/config/host_mappings/basic_auth_probe", {
      target,
    });
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
    await apiClient.post("/config/stream_mappings", { mappings });
  },
  async getSubdomainMode(): Promise<SubdomainModeConfig> {
    const res = await apiClient.get("/config/subdomain_mode");
    return res.data.data;
  },
  async updateSubdomainMode(config: Partial<SubdomainModeConfig>): Promise<
    SubdomainModeConfig & {
      ssl_auto_selection?: {
        applied: boolean;
        certificate_id?: string;
        label?: string;
        message: string;
      } | null;
    }
  > {
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
  async readSSLSharedFile(
    path: string,
  ): Promise<{ file: SharedDataFileEntry; content: string }> {
    const res = await apiClient.get("/ssl/shared-files/content", {
      params: { path },
    });
    return res.data.data;
  },
  // CA
  async getCAStatus(): Promise<{ initialized: boolean; info?: any }> {
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
    const res = await apiClient.post("/ssl/ca/hosts", { value });
    return res.data.data || [];
  },
  async removeCAHost(value: string): Promise<string[]> {
    const res = await apiClient.delete("/ssl/ca/hosts", { data: { value } });
    return res.data.data || [];
  },
  async clearCAHosts(): Promise<void> {
    await apiClient.delete("/ssl/ca/hosts", { data: { all: true } });
  },
  async issueAndInstall(): Promise<{ success: boolean; message?: string }> {
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
    deployment_mode: "single_active" | "multi_sni",
  ): Promise<SSLStatus> {
    const res = await apiClient.post("/ssl/deployment-mode", {
      deployment_mode,
    });
    return res.data.data;
  },
  async activateSSLCertificate(id: string): Promise<void> {
    await apiClient.post("/ssl/activate", { id });
  },
  async deleteSSLCertificate(id: string): Promise<void> {
    await apiClient.delete(`/ssl/certificates/${encodeURIComponent(id)}`);
  },
  async clearSSLCertificateLibrary(): Promise<void> {
    await apiClient.delete("/ssl/certificates");
  },
  async updateDefaultRoute(path: string): Promise<void> {
    await apiClient.post("/config/default_route", { path });
  },
  async getGatewaySettings(): Promise<GatewaySettings> {
    const res = await apiClient.get("/config/gateway");
    return res.data.data;
  },
  async updateGatewaySettings(
    payload: Partial<Omit<GatewaySettings, "portal">> & {
      portal?: Partial<GatewayPortalConfig>;
    },
  ): Promise<GatewaySettings> {
    const res = await apiClient.post("/config/gateway", payload);
    return res.data.data;
  },
  async getGatewayVisibility(): Promise<GatewayVisibilityDetails> {
    const res = await apiClient.get("/config/gateway/visibility");
    return res.data.data;
  },
  async updateGatewayVisibility(payload: {
    enabled: boolean;
    selections: Array<{
      province: string;
      query_city?: string | null;
    }>;
    custom_cidrs: string[];
  }): Promise<GatewayVisibilityDetails> {
    const res = await apiClient.post("/config/gateway/visibility", payload);
    return res.data.data;
  },
  async getGatewayProxyHeaders(): Promise<GatewayProxyHeadersDetails> {
    const res = await apiClient.get("/config/gateway/proxy-headers");
    return res.data.data;
  },
  async updateGatewayProxyHeaders(payload: {
    disabled_hosts: string[];
  }): Promise<GatewayProxyHeadersDetails> {
    const res = await apiClient.post("/config/gateway/proxy-headers", payload);
    return res.data.data;
  },
  async getGatewayHostResponse(): Promise<GatewayHostResponseDetails> {
    const res = await apiClient.get("/config/gateway/host-response");
    return res.data.data;
  },
  async updateGatewayHostResponse(payload: {
    disabled_hosts: string[];
  }): Promise<GatewayHostResponseDetails> {
    const res = await apiClient.post("/config/gateway/host-response", payload);
    return res.data.data;
  },
  async getProxyProtocolForce(): Promise<ProxyProtocolForce> {
    const res = await apiClient.get("/config/proxy_protocol_force");
    return res.data.data;
  },
  async setProxyProtocolForce(
    proxy_protocol_force: boolean,
  ): Promise<ProxyProtocolForce> {
    const res = await apiClient.post("/config/proxy_protocol_force", {
      proxy_protocol_force,
    });
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
  async createAuthAccount(payload: {
    username: string;
    password: string;
  }): Promise<AuthAccount> {
    const res = await apiClient.post("/auth/accounts", payload);
    return res.data.data;
  },
  async updateAuthAccount(
    id: string,
    payload: { username?: string },
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
    payload: { username: string; password: string },
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
  async createOIDCProvider(payload: {
    name?: string;
    type: string;
    enabled?: boolean;
    connection_config?: Record<string, unknown>;
  }): Promise<OIDCProviderView> {
    const res = await apiClient.post("/auth/oidc/providers", payload);
    return res.data.data;
  },
  async updateOIDCProvider(
    id: string,
    payload: {
      name?: string;
      enabled?: boolean;
      connection_config?: Record<string, unknown>;
    },
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
  async testOIDCProvider(
    id: string,
  ): Promise<{ success: boolean; message?: string }> {
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
  async createOIDCInvite(payload: {
    totp_id: string;
    provider_id: string;
    note?: string;
  }): Promise<{ invite_url: string; expires_at: string }> {
    const res = await apiClient.post("/auth/oidc/invitations", payload);
    return res.data.data;
  },
  // Sync routes.
  async syncRoutes(): Promise<{
    success: boolean;
    message?: string;
    data?: {
      synced_rules: number;
      synced_host_rules?: number;
      synced_stream_rules?: number;
    };
  }> {
    const res = await apiClient.post("/sync-routes");
    return res.data;
  },
};

export const MaintenanceAPI = {
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
};

export type AccessEntryInfo = {
  env: "GO_REPROXY_PORT" | "FRP_REMOTE_PORT";
  port: string;
  isDefault: boolean;
};

export type RunModePromptPreferences = {
  directToReverseProxy: boolean;
  reverseProxyToDirect: boolean;
  switchToSubdomain: boolean;
  subdomainToReverseProxy: boolean;
};

export type UpdateDownloadStatus =
  | "idle"
  | "downloading"
  | "verifying"
  | "downloaded"
  | "installing"
  | "error";

export type UpdateLatestPayload = {
  version: string;
  update_available: boolean;
  force_update: boolean;
  download_url: string;
  sha256: string;
  download_url_arm64: string;
  sha256_arm64: string;
  release_notes: string;
};

export type UpdateStatusPayload = {
  githubUrl: string;
  localVersion: string;
  latest: UpdateLatestPayload | null;
  updateEnabled: boolean;
  hasUpdate: boolean;
  forceUpdate: boolean;
  check: {
    lastCheckedAt: number | null;
    error: string | null;
  };
  download: {
    status: UpdateDownloadStatus;
    percent: number;
    downloadedBytes: number;
    totalBytes: number | null;
    error: string | null;
    targetVersion: string | null;
  };
};

export type UpdateConfirmPayload = {
  version: string;
  completedAt: string;
};

export type SystemClockIssueCode = "timezone_mismatch" | "time_mismatch";

export type SystemClockIssue = {
  code: SystemClockIssueCode;
  title: string;
  message: string;
};

export type SystemClockStatus = {
  expectedTimeZone: string;
  systemTimeZone: string | null;
  checkedAt: string | null;
  networkSource: string | null;
  hasRemoteTime: boolean;
  lastCheckError: string | null;
  systemTimeMs: number | null;
  remoteTimeMs: number | null;
  systemBeijingTime: string | null;
  remoteBeijingTime: string | null;
  driftMs: number | null;
  driftThresholdMs: number;
  timeMismatch: boolean;
  timezoneMismatch: boolean;
  needsAttention: boolean;
  issues: SystemClockIssue[];
  checking: boolean;
  syncInProgress: boolean;
  lastSyncAt: string | null;
  lastSyncError: string | null;
  syncSummary: string | null;
};

export const SystemAPI = {
  async getClockStatus(): Promise<SystemClockStatus> {
    const res = await apiClient.get("/system/clock/status");
    return res.data.data;
  },
  async refreshClockStatus(): Promise<SystemClockStatus> {
    const res = await apiClient.post("/system/clock/check");
    return res.data.data;
  },
  async syncClock(): Promise<{
    message: string;
    data: SystemClockStatus;
  }> {
    const res = await apiClient.post("/system/clock/sync");
    return {
      message: String(res.data.message || "System time synchronized"),
      data: res.data.data,
    };
  },
  async getAccessEntry(): Promise<AccessEntryInfo> {
    const res = await apiClient.get("/system/access-entry");
    return res.data.data;
  },
  async resetFirewallByRunType(run_type: RunType): Promise<{
    runType: RunType;
    gatewayPort: number;
    exemptPorts: string[];
    whitelistSynced: number;
  }> {
    const res = await apiClient.post("/firewall/reset", { run_type });
    return res.data.data;
  },
  async clearFirewall(): Promise<{
    gatewayPort: number;
  }> {
    const res = await apiClient.post("/firewall/clear");
    return res.data.data;
  },
  async getRunModePromptPreferences(): Promise<RunModePromptPreferences> {
    const res = await apiClient.get("/config/run_mode_prompt_preferences");
    return res.data.data;
  },
  async updateRunModePromptPreferences(
    payload: Partial<RunModePromptPreferences>,
  ): Promise<RunModePromptPreferences> {
    const res = await apiClient.post(
      "/config/run_mode_prompt_preferences",
      payload,
    );
    return res.data.data;
  },
  async getProtocolMappingFeatureConfig(): Promise<ProtocolMappingFeatureConfig> {
    const res = await apiClient.get("/config/protocol_mapping_feature");
    return res.data.data;
  },
  async updateProtocolMappingFeatureConfig(
    payload: Partial<ProtocolMappingFeatureConfig>,
  ): Promise<ProtocolMappingFeatureConfig> {
    const res = await apiClient.post(
      "/config/protocol_mapping_feature",
      payload,
    );
    return res.data.data;
  },
  async getAutoHttpsDetails(): Promise<AutoHttpsDetails> {
    const res = await apiClient.get("/config/auto_https");
    return res.data.data;
  },
  async updateAutoHttps(
    payload: Partial<AutoHttpsConfig>,
  ): Promise<AutoHttpsDetails> {
    const res = await apiClient.post("/config/auto_https", payload);
    return res.data.data;
  },
  async getSmartConnectDetails(): Promise<SmartConnectDetails> {
    const res = await apiClient.get("/config/smart_connect/details");
    return res.data.data;
  },
  async updateSmartConnect(
    payload: Partial<SmartConnectConfig>,
  ): Promise<SmartConnectDetails> {
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
  async getFnosShareBypassConfig(): Promise<FnosShareBypassConfig> {
    const res = await apiClient.get("/config/fnos_share_bypass");
    return res.data.data;
  },
  async updateFnosShareBypassConfig(
    payload: Partial<FnosShareBypassConfig>,
  ): Promise<FnosShareBypassConfig> {
    const res = await apiClient.post("/config/fnos_share_bypass", payload);
    return res.data.data;
  },
  async getFnosPortIconHijackConfig(): Promise<FnosPortIconHijackConfig> {
    const res = await apiClient.get("/config/fnos_port_icon_hijack");
    return res.data.data;
  },
  async updateFnosPortIconHijackConfig(
    payload: Partial<FnosPortIconHijackConfig>,
  ): Promise<FnosPortIconHijackConfig> {
    const res = await apiClient.post("/config/fnos_port_icon_hijack", payload);
    return res.data.data;
  },
  async getFnosNetworkTuningStatus(): Promise<FnosNetworkTuningStatus> {
    const res = await apiClient.get("/config/fnos_network_tuning");
    return res.data.data;
  },
  async updateFnosNetworkTuningConfig(
    payload: FnosNetworkTuningUpdatePayload,
  ): Promise<FnosNetworkTuningStatus> {
    const res = await apiClient.post("/config/fnos_network_tuning", payload);
    return res.data.data;
  },
  async getFrpStatus() {
    const res = await apiClient.get("/system/frp/status");
    return res.data;
  },
  async startFrpDownload() {
    const res = await apiClient.post("/system/frp/download");
    return res.data;
  },
  async cancelFrpDownload() {
    const res = await apiClient.post("/system/frp/cancel");
    return res.data;
  },
  async deleteFrp() {
    const res = await apiClient.delete("/system/frp");
    return res.data;
  },
  async getCloudflaredStatus() {
    const res = await apiClient.get("/system/cloudflared/status");
    return res.data;
  },
  async startCloudflaredDownload() {
    const res = await apiClient.post("/system/cloudflared/download");
    return res.data;
  },
  async cancelCloudflaredDownload() {
    const res = await apiClient.post("/system/cloudflared/cancel");
    return res.data;
  },
  async deleteCloudflared() {
    const res = await apiClient.delete("/system/cloudflared");
    return res.data;
  },
};

export const CaptchaAPI = {
  async getSettings(): Promise<CaptchaSettings> {
    const res = await apiClient.get("/config/captcha");
    return res.data.data;
  },
  async updateSettings(payload: CaptchaSettings): Promise<CaptchaSettings> {
    const res = await apiClient.post("/config/captcha", payload);
    return res.data.data;
  },
};

export type IpLocationApiMode = "online" | "custom";

export type IpLocationApiConfig = {
  ip_lookup_mode: IpLocationApiMode;
  ip_lookup_url: string;
  cidr_mode: IpLocationApiMode;
  cidr_url: string;
};

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
  async testIpLookup(
    url: string,
  ): Promise<{ success: boolean; message: string }> {
    const res = await apiClient.post("/config/ip_location_api/test-ip-lookup", {
      url,
    });
    return res.data;
  },
  async testCidr(url: string): Promise<{ success: boolean; message: string }> {
    const res = await apiClient.post("/config/ip_location_api/test-cidr", {
      url,
    });
    return res.data;
  },
};

export const UpdateAPI = {
  async getStatus(): Promise<UpdateStatusPayload> {
    const res = await apiClient.get("/update/status");
    return res.data.data;
  },
  async checkNow(): Promise<UpdateStatusPayload> {
    const res = await apiClient.post("/update/check");
    return res.data.data;
  },
  async checkAndDownload(): Promise<{
    success: boolean;
    message?: string;
    data?: UpdateStatusPayload;
  }> {
    const res = await apiClient.post("/update/check-and-download");
    return res.data;
  },
  async startDownload(): Promise<{
    success: boolean;
    message?: string;
    data?: UpdateStatusPayload;
  }> {
    const res = await apiClient.post("/update/download");
    return res.data;
  },
  async startInstall(): Promise<{ success: boolean; message?: string }> {
    const res = await apiClient.post("/update/install");
    return res.data;
  },
  async consumeConfirm(): Promise<UpdateConfirmPayload | null> {
    const res = await apiClient.get("/update/confirm");
    return res.data.data || null;
  },
};

export type BackoffItem = {
  ip: string;
  attempts: number;
  blocked: boolean;
  retryAfter?: number;
  blockedUntil?: number;
};

export const BackoffAPI = {
  async list(): Promise<BackoffItem[]> {
    const res = await apiClient.get("/backoff/list");
    return res.data.data || [];
  },
  async status(ip: string): Promise<BackoffItem> {
    const res = await apiClient.get("/backoff/status", { params: { ip } });
    return res.data.data;
  },
  async reset(ip: string): Promise<void> {
    await apiClient.post("/backoff/reset", { ip });
  },
};
