import type {
  AppConfig,
  HostMapping,
  HostMappingBasicAuth,
  HostMappingRefreshSummary,
  HostActiveIpsPayload,
  UrlMetadataPreview,
  PasskeyCredential,
  ProxyMapping,
  ProxyProtocolForce,
  ReverseProxySubmode,
  RunType,
  StreamMapping,
  SessionMobilityDetails,
  SessionRecord,
  SharedDataFileEntry,
  SSLConfig,
  SSLSharedFilesPayload,
  SSLStatus,
  SubdomainModeConfig,
  TOTPCredential,
  TOTPAccessScope,
  TrafficStats,
  DashboardStats,
  ThreatOverview,
  DockerAdminBootstrapState,
  SystemEventLevel,
  SystemEventListPayload,
  SystemEventSource,
  SystemEventType,
  FnosPortIconHijackConfig,
  FnosShareBypassConfig,
  GatewayHostResponseDetails,
  GatewayProxyHeadersDetails,
  GatewaySettings,
  GatewayPortalConfig,
  GatewayVisibilityDetails,
  FnKnockBackupImportArchiveRequest,
  FnKnockBackupImportResult,
  BackupDirectoryFilesPayload,
  FnKnockBackupExportToDirectoryResult,
  GatewayLogDatesPayload,
  GatewayLogDeletePayload,
  GatewayLogEntriesPayload,
  GatewayLoggingConfig,
  WAFConfig,
  WAFDetails,
  WAFDrainResult,
  WAFLogDeletePayload,
  WAFLogEntriesPayload,
  WAFRuleFileContent,
  WAFStatus,
  DashboardDisplayConfig,
  IpLocationBatchPayload,
  IpLocationSnapshot,
  LocaleConfig,
  AutoHttpsConfig,
  AutoHttpsDetails,
  ProtocolMappingFeatureConfig,
  SmartConnectConfig,
  SmartConnectDetails,
  DnsmasqStatus,
  DnsmasqInstallState,
  AuthCredentialSettings,
  TerminalAttachmentRecord,
  TerminalFeatureConfig,
  TerminalOutputChunk,
  TerminalRuntimeStatus,
  TerminalSessionRecord,
  TerminalTmuxInstallState,
  CidrCitiesPayload,
  CidrLookupPayload,
  CidrProvincesPayload,
  CidrSelectorPayload,
  NotificationDeliveryListPayload,
  NotificationDeliveryStatus,
  NotificationProviderCatalogPayload,
  NotificationProviderDetailView,
  NotificationProviderListPayload,
  NotificationRuleListPayload,
  NotificationTriggerListPayload,
  NotificationTriggerStatus,
  OIDCBinding,
  OIDCProviderCatalogItem,
  OIDCProviderView,
  SSHLoginLogListPayload,
  SSHSecurityBlockListPayload,
  SSHSecurityBlockRecord,
  SSHSecurityConfig,
  SSHSecurityDetails,
  SSHSecurityFirewallClearResult,
  SSHSecurityFirewallSyncResult,
  WelcomeGuideStatus,
} from "../types";
import { createApiClient } from "@frontend-core/api/createApiClient";
import type { CaptchaSettings } from "@frontend-core/captcha/types";

type HostMappingUpdatePayload = Pick<
  HostMapping,
  | "host"
  | "target"
  | "use_auth"
  | "access_mode"
  | "suppress_toolbar"
  | "preserve_host"
  | "basic_auth"
  | "locations"
  | "title_override"
>;

export interface HostMappingBasicAuthProbeResult {
  requiresBasicAuth: boolean;
  httpStatus: number | null;
  error?: string;
}

const resolveAppRelativePath = (relativePath: string) => {
  if (typeof window === "undefined") return relativePath;
  const basePath = window.location.pathname.endsWith("/")
    ? window.location.pathname
    : `${window.location.pathname}/`;
  return new URL(relativePath, `${window.location.origin}${basePath}`).pathname;
};

const adminApiBasePath = resolveAppRelativePath("./api/admin");

export const apiClient = createApiClient({
  baseURL: adminApiBasePath,
  withCredentials: true,
});

apiClient.interceptors.response.use(
  (response) => response,
  (error) => {
    if (typeof window !== "undefined" && error?.response?.status === 401) {
      window.dispatchEvent(
        new CustomEvent("fn-knock:docker-admin-auth-required"),
      );
    }
    return Promise.reject(error);
  },
);

const toHostMappingBasicAuthPayload = (
  basicAuth: HostMappingBasicAuth,
): HostMappingBasicAuth => ({
  enabled: basicAuth.enabled,
  username: basicAuth.username.trim(),
  password: basicAuth.password,
});

const toHostMappingUpdatePayload = (
  mapping: HostMapping,
): HostMappingUpdatePayload => ({
  host: mapping.host,
  target: mapping.target,
  use_auth: mapping.use_auth,
  access_mode: mapping.access_mode,
  suppress_toolbar: mapping.suppress_toolbar,
  preserve_host: mapping.preserve_host,
  basic_auth: toHostMappingBasicAuthPayload(mapping.basic_auth),
  locations: (mapping.locations ?? []).map((location) => ({
    path: location.path.trim(),
    match: location.match,
    action: location.action,
    target: location.target.trim(),
    strip_path: location.strip_path,
    rewrite_html: location.rewrite_html,
    response: {
      status: location.response.status,
      content_type: location.response.content_type.trim(),
      headers: { ...location.response.headers },
      body: location.response.body,
    },
  })),
  title_override: mapping.title_override.trim(),
});

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

export interface WhiteListRecord {
  id: string;
  ip: string;
  targetType: "ip" | "cidr" | "cname";
  expireAt: number | null;
  source: "manual" | "auto";
  createdAt: number;
  comment?: string;
  status: "active" | "expired" | "deleted";
  ipLocation?: string;
  resolvedTargets?: string[];
  checkIntervalMinutes?: number | null;
  lastCheckedAt?: number | null;
  lastResolvedAt?: number | null;
  resolveStatus?: "pending" | "resolved" | "empty" | "error";
  resolveMessage?: string;
}

export const WhitelistAPI = {
  async getRecords() {
    const res = await apiClient.get("/whitelist");
    return res.data;
  },
  async addRecord(payload: {
    ip: string;
    targetType?: "ip" | "cidr" | "cname";
    expireAt: number | null;
    source: string;
    comment?: string;
    checkIntervalMinutes?: number;
  }) {
    const res = await apiClient.post("/whitelist", payload);
    return res.data;
  },
  async deleteRecord(id: string) {
    const res = await apiClient.delete(`/whitelist/${encodeURIComponent(id)}`);
    return res.data;
  },
  async updateComment(id: string, comment: string) {
    const res = await apiClient.patch(
      `/whitelist/${encodeURIComponent(id)}/comment`,
      { comment },
    );
    return res.data;
  },
  async refreshRecord(id: string): Promise<{
    success: boolean;
    message?: string;
    data?: {
      changed: boolean;
      skipped: boolean;
      record: WhiteListRecord;
    };
  }> {
    const res = await apiClient.post(
      `/whitelist/${encodeURIComponent(id)}/refresh`,
    );
    return res.data;
  },
};

export const EventCenterAPI = {
  async getEvents(params: {
    page: number;
    limit: string;
    search: string;
    type?: SystemEventType | "all";
    level?: SystemEventLevel | "all";
    source?: SystemEventSource | "all";
  }): Promise<{
    success: boolean;
    data: SystemEventListPayload;
    message?: string;
  }> {
    const res = await apiClient.get("/events", {
      params: {
        page: params.page,
        limit: params.limit,
        search: params.search,
        type: params.type && params.type !== "all" ? params.type : undefined,
        level:
          params.level && params.level !== "all" ? params.level : undefined,
        source:
          params.source && params.source !== "all" ? params.source : undefined,
      },
    });
    return res.data;
  },
  async deleteEvents(ids: string[]) {
    const res = await apiClient.delete("/events", { data: { ids } });
    return res.data;
  },
  async clearEvents(): Promise<{
    success: boolean;
    data: { deleted_count: number };
    message?: string;
  }> {
    const res = await apiClient.delete("/events/clear");
    return res.data;
  },
  async getNotificationProviderCatalog(): Promise<{
    success: boolean;
    data: NotificationProviderCatalogPayload;
    message?: string;
  }> {
    const res = await apiClient.get("/notifications/providers/catalog");
    return res.data;
  },
  async getNotificationProviders(): Promise<{
    success: boolean;
    data: NotificationProviderListPayload;
    message?: string;
  }> {
    const res = await apiClient.get("/notifications/providers");
    return res.data;
  },
  async getNotificationProvider(id: string): Promise<{
    success: boolean;
    data: NotificationProviderDetailView;
    message?: string;
  }> {
    const res = await apiClient.get(
      `/notifications/providers/${encodeURIComponent(id)}`,
    );
    return res.data;
  },
  async createNotificationProvider(payload: {
    name?: string;
    type: string;
    enabled: boolean;
    connection_config: Record<string, unknown>;
  }) {
    const res = await apiClient.post("/notifications/providers", payload);
    return res.data;
  },
  async updateNotificationProvider(
    id: string,
    payload: {
      name?: string;
      enabled?: boolean;
      connection_config?: Record<string, unknown>;
    },
  ) {
    const res = await apiClient.patch(
      `/notifications/providers/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data;
  },
  async deleteNotificationProvider(id: string) {
    const res = await apiClient.delete(
      `/notifications/providers/${encodeURIComponent(id)}`,
    );
    return res.data;
  },
  async testNotificationProvider(id: string) {
    const res = await apiClient.post(
      `/notifications/providers/${encodeURIComponent(id)}/test`,
    );
    return res.data;
  },
  async testNotificationProviderDraft(payload: {
    id?: string;
    name?: string;
    type: string;
    enabled: boolean;
    connection_config: Record<string, unknown>;
  }) {
    const res = await apiClient.post("/notifications/providers/test", payload);
    return res.data;
  },
  async getNotificationRules(): Promise<{
    success: boolean;
    data: NotificationRuleListPayload;
    message?: string;
  }> {
    const res = await apiClient.get("/notifications/rules");
    return res.data;
  },
  async createNotificationRule(payload: Record<string, unknown>) {
    const res = await apiClient.post("/notifications/rules", payload);
    return res.data;
  },
  async updateNotificationRule(id: string, payload: Record<string, unknown>) {
    const res = await apiClient.patch(
      `/notifications/rules/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data;
  },
  async deleteNotificationRule(id: string) {
    const res = await apiClient.delete(
      `/notifications/rules/${encodeURIComponent(id)}`,
    );
    return res.data;
  },
  async getNotificationTriggers(params: {
    page: number;
    limit: number;
    rule_id?: string;
    status?: NotificationTriggerStatus | "all";
  }): Promise<{
    success: boolean;
    data: NotificationTriggerListPayload;
    message?: string;
  }> {
    const res = await apiClient.get("/notifications/triggers", {
      params: {
        page: params.page,
        limit: params.limit,
        rule_id: params.rule_id || undefined,
        status:
          params.status && params.status !== "all" ? params.status : undefined,
      },
    });
    return res.data;
  },
  async getNotificationDeliveries(params: {
    page: number;
    limit: number;
    rule_id?: string;
    provider_id?: string;
    trigger_id?: string;
    status?: NotificationDeliveryStatus | "all";
  }): Promise<{
    success: boolean;
    data: NotificationDeliveryListPayload;
    message?: string;
  }> {
    const res = await apiClient.get("/notifications/deliveries", {
      params: {
        page: params.page,
        limit: params.limit,
        rule_id: params.rule_id || undefined,
        provider_id: params.provider_id || undefined,
        trigger_id: params.trigger_id || undefined,
        status:
          params.status && params.status !== "all" ? params.status : undefined,
      },
    });
    return res.data;
  },
  async clearNotificationDeliveries(params: {
    rule_id?: string;
    provider_id?: string;
    trigger_id?: string;
    status?: NotificationDeliveryStatus | "all";
  }) {
    const res = await apiClient.delete("/notifications/deliveries", {
      data: {
        rule_id: params.rule_id || undefined,
        provider_id: params.provider_id || undefined,
        trigger_id: params.trigger_id || undefined,
        status:
          params.status && params.status !== "all" ? params.status : undefined,
      },
    });
    return res.data as {
      success: boolean;
      data: {
        deleted_count: number;
      };
      message?: string;
    };
  },
};

export const GatewayLogsAPI = {
  async getConfig(): Promise<GatewayLoggingConfig> {
    const res = await apiClient.get("/gateway-logs/config");
    return res.data.data;
  },
  async updateConfig(
    payload: Pick<GatewayLoggingConfig, "enabled" | "max_days">,
  ): Promise<GatewayLoggingConfig> {
    const res = await apiClient.post("/gateway-logs/config", payload);
    return res.data.data;
  },
  async getDirectory(): Promise<{ logs_dir: string }> {
    const res = await apiClient.get("/gateway-logs/directory");
    return res.data.data;
  },
  async getDates(): Promise<GatewayLogDatesPayload> {
    const res = await apiClient.get("/gateway-logs/dates");
    return res.data.data;
  },
  async getEntries(params: {
    date: string;
    pagination: "page" | "cursor";
    limit: string;
    cursor?: string;
    search?: string;
    status?: string;
    logged_in?: string;
    waf_status?: string;
    page?: number;
  }): Promise<GatewayLogEntriesPayload> {
    const res = await apiClient.get("/gateway-logs/entries", {
      params,
    });
    return res.data.data;
  },
  async deleteDate(date: string): Promise<GatewayLogDeletePayload> {
    const res = await apiClient.delete("/gateway-logs/entries", {
      data: { date },
    });
    return res.data.data;
  },
};

export const WAFAPI = {
  async getDetails(): Promise<WAFDetails> {
    const res = await apiClient.get("/waf/details");
    return res.data.data;
  },
  async getStatus(): Promise<WAFStatus> {
    const res = await apiClient.get("/waf/status");
    return res.data.data;
  },
  async updateConfig(
    payload: Partial<
      Pick<
        WAFConfig,
        | "enabled"
        | "system_rules_auto_update_enabled"
        | "common_location_exempt_enabled"
        | "paranoia_level"
        | "executing_paranoia_level"
      >
    >,
  ): Promise<WAFDetails> {
    const res = await apiClient.post("/waf/config", payload);
    return res.data.data;
  },
  async refreshManifest(): Promise<WAFDetails> {
    const res = await apiClient.post("/waf/manifest/refresh");
    return res.data.data;
  },
  async syncSystemRules(): Promise<WAFDetails> {
    const res = await apiClient.post("/waf/system/sync");
    return res.data.data;
  },
  async setRulesEnabled(payload: {
    source: "system" | "custom";
    filenames?: string[];
    enabled: boolean;
  }): Promise<WAFDetails> {
    const res = await apiClient.post("/waf/rules/enabled", payload);
    return res.data.data;
  },
  async getRuleFile(
    source: "system" | "custom",
    filename: string,
  ): Promise<WAFRuleFileContent> {
    const res = await apiClient.get(
      `/waf/rules/${source}/${encodeURIComponent(filename)}`,
    );
    return res.data.data;
  },
  async uploadCustomRules(payload: {
    files: Array<{ filename: string; content_base64: string }>;
  }): Promise<WAFDetails> {
    const res = await apiClient.post("/waf/custom/upload", payload);
    return res.data.data;
  },
  async deleteCustomRule(filename: string): Promise<WAFDetails> {
    const res = await apiClient.delete(
      `/waf/custom/${encodeURIComponent(filename)}`,
    );
    return res.data.data;
  },
  async drainEvents(): Promise<WAFDrainResult> {
    const res = await apiClient.post("/waf/events/drain");
    return res.data.data;
  },
  async getLogs(params: {
    date?: string;
    trace_id?: string;
    search?: string;
    host?: string;
    client_ip?: string;
    rule_id?: string;
    route_type?: string;
    mode?: string;
    cursor?: string;
    limit?: string;
  }): Promise<WAFLogEntriesPayload> {
    const res = await apiClient.get("/waf/logs", { params });
    return res.data.data;
  },
  async getLog(
    traceId: string,
  ): Promise<WAFLogEntriesPayload["items"][number]> {
    const res = await apiClient.get(`/waf/logs/${encodeURIComponent(traceId)}`);
    return res.data.data;
  },
  async deleteLogs(date: string): Promise<WAFLogDeletePayload> {
    const res = await apiClient.delete("/waf/logs", {
      data: { date },
    });
    return res.data.data;
  },
};

const IP_LOCATION_BATCH_LIMIT = 20;

export const IpLocationAPI = {
  async lookupBatch(ips: string[]): Promise<IpLocationSnapshot[]> {
    if (ips.length === 0) return [];

    const tasks: Promise<IpLocationSnapshot[]>[] = [];
    for (let index = 0; index < ips.length; index += IP_LOCATION_BATCH_LIMIT) {
      const batch = ips.slice(index, index + IP_LOCATION_BATCH_LIMIT);
      tasks.push(
        apiClient
          .post("/ip-location/batch", { ips: batch })
          .then(
            (res) =>
              ((res.data.data as IpLocationBatchPayload).items ||
                []) as IpLocationSnapshot[],
          ),
      );
    }

    const groups = await Promise.all(tasks);
    return groups.flat();
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

export const TerminalAPI = {
  async getStatus(): Promise<TerminalRuntimeStatus> {
    const res = await apiClient.get("/terminal/status");
    return res.data.data;
  },
  async installTmux(): Promise<TerminalTmuxInstallState> {
    const res = await apiClient.post("/terminal/tmux/install");
    return res.data.data;
  },
  async listSessions(): Promise<TerminalSessionRecord[]> {
    const res = await apiClient.get("/terminal/sessions");
    return res.data.data;
  },
  async getSession(id: string): Promise<TerminalSessionRecord> {
    const res = await apiClient.get(
      `/terminal/sessions/${encodeURIComponent(id)}`,
    );
    return res.data.data;
  },
  async createSession(payload: {
    title?: string;
    shell?: string;
    cwd?: string;
    cols?: number;
    rows?: number;
  }): Promise<TerminalSessionRecord> {
    const res = await apiClient.post("/terminal/sessions", payload);
    return res.data.data;
  },
  async updateSessionTitle(
    id: string,
    title: string,
  ): Promise<TerminalSessionRecord> {
    const res = await apiClient.patch(
      `/terminal/sessions/${encodeURIComponent(id)}`,
      { title },
    );
    return res.data.data;
  },
  async deleteSession(id: string): Promise<void> {
    await apiClient.delete(`/terminal/sessions/${encodeURIComponent(id)}`);
  },
  async createAttachment(sessionId: string): Promise<TerminalAttachmentRecord> {
    const res = await apiClient.post(
      `/terminal/sessions/${encodeURIComponent(sessionId)}/attachments`,
    );
    return res.data.data;
  },
  async pollAttachment(
    attachmentId: string,
    params: { cursor?: number; timeout_ms?: number } = {},
  ): Promise<{ changed: boolean; chunk: TerminalOutputChunk | null }> {
    const res = await apiClient.get(
      `/terminal/attachments/${encodeURIComponent(attachmentId)}/poll`,
      { params },
    );
    return res.data.data;
  },
  async sendInput(attachmentId: string, dataBase64: string): Promise<void> {
    await apiClient.post(
      `/terminal/attachments/${encodeURIComponent(attachmentId)}/input`,
      { dataBase64 },
    );
  },
  async resizeAttachment(
    attachmentId: string,
    cols: number,
    rows: number,
  ): Promise<TerminalSessionRecord> {
    const res = await apiClient.post(
      `/terminal/attachments/${encodeURIComponent(attachmentId)}/resize`,
      { cols, rows },
    );
    return res.data.data;
  },
  async detachAttachment(attachmentId: string): Promise<void> {
    await apiClient.delete(
      `/terminal/attachments/${encodeURIComponent(attachmentId)}`,
    );
  },
};

export const SecurityAPI = {
  async getOverview(rangeSec: number): Promise<ThreatOverview> {
    const res = await apiClient.get("/security/overview", {
      params: { rangeSec },
    });
    return res.data.data;
  },
};

export type ScannerSettings = {
  enabled: boolean;
  windowMinutes: number;
  threshold: number;
  windowSeconds: number;
  blacklistTtlSeconds: number;
  commonLocationExemptEnabled: boolean;
};

export type ScannerBlacklistHit = {
  path: string;
  createdAt: number;
};

export type ScannerBlacklistRecord = {
  ip: string;
  ipLocation?: string;
  blockedAt: number;
  windowMinutes: number;
  threshold: number;
  hits: ScannerBlacklistHit[];
};

export type ScannerBlacklistList = {
  items: ScannerBlacklistRecord[];
  total: number;
};

export type GeneralBlacklistSource =
  | "manual"
  | "request_log"
  | "active_ip"
  | "waf_log";

export type GeneralBlacklistRecord = {
  ip: string;
  source?: GeneralBlacklistSource | string;
  comment?: string;
  created_at?: string;
  updated_at?: string;
  ipLocation?: string;
};

export type GeneralBlacklistList = {
  items: GeneralBlacklistRecord[];
  total: number;
};

export type GeneralBlacklistMutationResult = {
  added: number;
  updated: number;
  removed: number;
  total: number;
  items: GeneralBlacklistRecord[];
};

export type GeneralBlacklistStatus = {
  records: Record<string, GeneralBlacklistRecord>;
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
  async getTrafficStats(): Promise<TrafficStats> {
    const res = await apiClient.get("/traffic");
    return res.data.data;
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

export const ScannerAPI = {
  async getSettings(): Promise<ScannerSettings> {
    const res = await apiClient.get("/scanner/settings");
    return res.data.data;
  },
  async saveSettings(payload: {
    enabled: boolean;
    windowMinutes: number;
    threshold: number;
    blacklistTtlSeconds: number;
    commonLocationExemptEnabled?: boolean;
  }): Promise<ScannerSettings> {
    const res = await apiClient.post("/scanner/settings", payload);
    return res.data.data;
  },
  async getBlacklist(
    page: number,
    limit: string,
    search: string,
  ): Promise<ScannerBlacklistList> {
    const res = await apiClient.get("/scanner/blacklist", {
      params: { page, limit, search },
    });
    return res.data.data;
  },
  async getBlacklistDetail(ip: string): Promise<ScannerBlacklistRecord> {
    const res = await apiClient.get(
      `/scanner/blacklist/${encodeURIComponent(ip)}`,
    );
    return res.data.data;
  },
  async deleteBlacklist(ips: string[]): Promise<void> {
    await apiClient.delete("/scanner/blacklist", { data: { ips } });
  },
  async deleteBlacklistByIp(ip: string): Promise<void> {
    await apiClient.delete(`/scanner/blacklist/${encodeURIComponent(ip)}`);
  },
};

export const GeneralBlacklistAPI = {
  async getList(
    page: number,
    limit: string,
    search: string,
  ): Promise<GeneralBlacklistList> {
    const res = await apiClient.get("/general-blacklist", {
      params: { page, limit, search },
    });
    return res.data.data;
  },
  async add(
    ips: string[],
    source: GeneralBlacklistSource,
    comment?: string,
  ): Promise<GeneralBlacklistMutationResult> {
    const res = await apiClient.post("/general-blacklist", {
      ips,
      source,
      comment,
    });
    return res.data.data;
  },
  async getStatus(ips: string[]): Promise<GeneralBlacklistStatus> {
    const res = await apiClient.post("/general-blacklist/status", { ips });
    return res.data.data;
  },
  async delete(ips: string[]): Promise<GeneralBlacklistMutationResult> {
    const res = await apiClient.delete("/general-blacklist", { data: { ips } });
    return res.data.data;
  },
  async deleteByIp(ip: string): Promise<GeneralBlacklistMutationResult> {
    const res = await apiClient.delete(
      `/general-blacklist/${encodeURIComponent(ip)}`,
    );
    return res.data.data;
  },
};

export const SSHSecurityAPI = {
  async getDetails(): Promise<SSHSecurityDetails> {
    const res = await apiClient.get("/ssh-security/config");
    return res.data.data;
  },
  async updateConfig(
    payload: Partial<Omit<SSHSecurityConfig, "allowed_regions">> & {
      allowed_regions?: Array<{
        province: string;
        query_city?: string | null;
      }>;
    },
  ): Promise<SSHSecurityDetails> {
    const res = await apiClient.post("/ssh-security/config", payload);
    return res.data.data;
  },
  async syncFirewall(): Promise<SSHSecurityFirewallSyncResult> {
    const res = await apiClient.post("/ssh-security/firewall/sync");
    return res.data.data;
  },
  async clearFirewall(): Promise<SSHSecurityFirewallClearResult> {
    const res = await apiClient.post("/ssh-security/firewall/clear");
    return res.data.data;
  },
  async getLoginLogs(params: {
    page: number;
    limit: string;
    search?: string;
    outcome?: "success" | "failure" | "all";
  }): Promise<SSHLoginLogListPayload> {
    const res = await apiClient.get("/ssh-security/login-logs", {
      params: {
        page: params.page,
        limit: params.limit,
        search: params.search || undefined,
        outcome:
          params.outcome && params.outcome !== "all"
            ? params.outcome
            : undefined,
      },
    });
    return res.data.data;
  },
  async getBlocks(
    page: number,
    limit: string,
    search: string,
  ): Promise<SSHSecurityBlockListPayload> {
    const res = await apiClient.get("/ssh-security/blocks", {
      params: { page, limit, search },
    });
    return res.data.data;
  },
  async getBlock(ip: string): Promise<SSHSecurityBlockRecord> {
    const res = await apiClient.get(
      `/ssh-security/blocks/${encodeURIComponent(ip)}`,
    );
    return res.data.data;
  },
  async deleteBlock(ip: string): Promise<void> {
    await apiClient.delete(`/ssh-security/blocks/${encodeURIComponent(ip)}`);
  },
  async deleteBlocks(ips: string[]): Promise<void> {
    await apiClient.delete("/ssh-security/blocks", { data: { ips } });
  },
};

export const FrpcAPI = {
  async getStatus(): Promise<{
    initialized: boolean;
    platform: string;
    running: boolean;
    pid: number | null;
    config_path: string;
    defaults: { local_port: string };
  }> {
    const res = await apiClient.get("/frpc/status");
    return res.data.data;
  },
  async getOverview(
    limit = 200,
  ): Promise<{ tcp: FrpcTcpItem[]; logs: string[] }> {
    const res = await apiClient.get("/frpc/overview", { params: { limit } });
    return res.data.data;
  },
  async getWebStatus(): Promise<{ tcp: FrpcTcpItem[] }> {
    const res = await apiClient.get("/frpc/web-status");
    return res.data.data;
  },
  async getConfig(): Promise<string> {
    const res = await apiClient.get("/frpc/config");
    return res.data.data.content as string;
  },
  async saveConfig(content: string): Promise<void> {
    await apiClient.post("/frpc/config", { content });
  },
  async start(): Promise<{ pid: number }> {
    const res = await apiClient.post("/frpc/start");
    return res.data.data;
  },
  async stop(): Promise<void> {
    await apiClient.post("/frpc/stop");
  },
  async getLogs(limit = 200): Promise<string[]> {
    const res = await apiClient.get("/frpc/logs", { params: { limit } });
    return res.data.data as string[];
  },
  async clearLogs(): Promise<void> {
    await apiClient.delete("/frpc/logs");
  },
  async poll(cursor?: number): Promise<FrpcPollPayload> {
    const res = await apiClient.get("/frpc/poll", {
      params: typeof cursor === "number" ? { cursor } : undefined,
    });
    return res.data.data;
  },
  async getInstances(): Promise<FrpcInstancesOverview> {
    const res = await apiClient.get("/frpc/instances");
    return res.data.data;
  },
  async createDraft(): Promise<string> {
    const res = await apiClient.post("/frpc/instances/draft");
    return res.data.data.content as string;
  },
  async createInstance(payload: {
    name?: string;
    content?: string;
  }): Promise<FrpcInstanceStatus> {
    const res = await apiClient.post("/frpc/instances", payload);
    return res.data.data;
  },
  async getInstance(id: string, limit = 200): Promise<FrpcInstanceDetail> {
    const res = await apiClient.get(
      `/frpc/instances/${encodeURIComponent(id)}`,
      { params: { limit } },
    );
    return res.data.data;
  },
  async updateInstance(
    id: string,
    payload: { name?: string; content?: string },
  ): Promise<FrpcInstanceStatus> {
    const res = await apiClient.put(
      `/frpc/instances/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data.data;
  },
  async deleteInstance(id: string): Promise<void> {
    await apiClient.delete(`/frpc/instances/${encodeURIComponent(id)}`);
  },
  async startInstance(id: string): Promise<{ pid: number }> {
    const res = await apiClient.post(
      `/frpc/instances/${encodeURIComponent(id)}/start`,
    );
    return res.data.data;
  },
  async stopInstance(id: string): Promise<void> {
    await apiClient.post(`/frpc/instances/${encodeURIComponent(id)}/stop`);
  },
  async restartInstance(id: string): Promise<{ pid: number }> {
    const res = await apiClient.post(
      `/frpc/instances/${encodeURIComponent(id)}/restart`,
    );
    return res.data.data;
  },
  async getInstanceLogs(id: string, limit = 200): Promise<string[]> {
    const res = await apiClient.get(
      `/frpc/instances/${encodeURIComponent(id)}/logs`,
      { params: { limit } },
    );
    return res.data.data as string[];
  },
  async clearInstanceLogs(id: string): Promise<void> {
    await apiClient.delete(`/frpc/instances/${encodeURIComponent(id)}/logs`);
  },
  async pollInstance(
    id: string,
    cursor?: number,
  ): Promise<FrpcInstancePollPayload> {
    const res = await apiClient.get(
      `/frpc/instances/${encodeURIComponent(id)}/poll`,
      { params: typeof cursor === "number" ? { cursor } : undefined },
    );
    return res.data.data;
  },
};

export type CloudflaredProtocol = "auto" | "http2" | "quic";

export type CloudflaredConfig = {
  token: string;
  protocol: CloudflaredProtocol;
};

export const CloudflaredAPI = {
  async getStatus(): Promise<{
    initialized: boolean;
    platform: string;
    running: boolean;
    pid: number | null;
  }> {
    const res = await apiClient.get("/cloudflared/status");
    return res.data.data;
  },
  async getConfig(): Promise<CloudflaredConfig> {
    const res = await apiClient.get("/cloudflared/config");
    return res.data.data;
  },
  async saveConfig(config: CloudflaredConfig): Promise<void> {
    await apiClient.post("/cloudflared/config", config);
  },
  async start(): Promise<{ pid: number }> {
    const res = await apiClient.post("/cloudflared/start");
    return res.data.data;
  },
  async stop(): Promise<void> {
    await apiClient.post("/cloudflared/stop");
  },
  async getLogs(limit = 200): Promise<string[]> {
    const res = await apiClient.get("/cloudflared/logs", { params: { limit } });
    return res.data.data as string[];
  },
  async clearLogs(): Promise<void> {
    await apiClient.delete("/cloudflared/logs");
  },
  async poll(cursor?: number): Promise<CloudflaredPollPayload> {
    const res = await apiClient.get("/cloudflared/poll", {
      params: typeof cursor === "number" ? { cursor } : undefined,
    });
    return res.data.data;
  },
};

export type PollTarget = "dashboard" | "ddns" | "frpc" | "cloudflared";

export type PollingPayloadMap = {
  dashboard: TrafficStats;
  ddns: DDNSPollPayload;
  frpc: FrpcPollPayload;
  cloudflared: CloudflaredPollPayload;
};

export const PollingAPI = {
  async poll<T extends PollTarget>(
    target: T,
    cursor?: number,
  ): Promise<PollingPayloadMap[T]> {
    switch (target) {
      case "dashboard":
        return (await DashboardAPI.getRealtime()) as PollingPayloadMap[T];
      case "ddns":
        return (await DDNSAPI.poll(cursor)) as PollingPayloadMap[T];
      case "frpc":
        return (await FrpcAPI.poll(cursor)) as PollingPayloadMap[T];
      case "cloudflared":
        return (await CloudflaredAPI.poll(cursor)) as PollingPayloadMap[T];
      default:
        throw new Error(`Unsupported poll target: ${String(target)}`);
    }
  },
};

export const SessionAPI = {
  async list(): Promise<SessionRecord[]> {
    const res = await apiClient.get("/sessions");
    return Array.isArray(res.data?.data) ? res.data.data : [];
  },
  async get(id: string): Promise<SessionRecord> {
    const res = await apiClient.get(`/sessions/${encodeURIComponent(id)}`);
    return res.data.data;
  },
  async getMobility(id: string): Promise<SessionMobilityDetails> {
    const res = await apiClient.get(
      `/sessions/${encodeURIComponent(id)}/mobility`,
    );
    return res.data.data;
  },
  async updateComment(id: string, comment: string): Promise<SessionRecord> {
    const res = await apiClient.patch(
      `/sessions/${encodeURIComponent(id)}/comment`,
      { comment },
    );
    return res.data.data;
  },
  async kick(id: string): Promise<void> {
    await apiClient.delete(`/sessions/${encodeURIComponent(id)}`);
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

export type AcmeCertificateAuthority = "zerossl" | "letsencrypt";
export type AcmeJobStatus =
  | "queued"
  | "running"
  | "succeeded"
  | "failed"
  | "stopped";
export type AcmeJobTrigger = "manual_request" | "auto_renew";

export type AcmeDnsProvider = {
  dnsType: string;
  label: string;
  group: string;
  credentialSchemes: Array<{
    id: string;
    label: string;
    description?: string;
    fields: Array<{
      key: string;
      label?: string;
      description?: string;
      required?: boolean;
    }>;
  }>;
};

export type AcmeLogAnalysis = {
  reason:
    | "dns_credentials_invalid"
    | "dns_credentials_invalid_email"
    | "dns_api_rate_limited"
    | "acme_frequency_limited"
    | "unknown";
  provider?: string;
  message: string;
  evidence?: string[];
};

export type AcmeJobData = {
  id: string;
  applicationId?: string;
  domains: string[];
  method: string;
  provider: string | null;
  trigger?: AcmeJobTrigger;
  createdAt: string;
  startedAt?: string;
  finishedAt?: string;
  status: AcmeJobStatus;
  progress: number;
  message?: string;
};

export type AcmeApplicationRecord = {
  id: string;
  name?: string;
  domains: string[];
  primaryDomain: string;
  dnsType: string;
  credentials: Record<string, string>;
  renewEnabled: boolean;
  createdAt: string;
  updatedAt: string;
  latestJobId?: string;
  latestJobStatus?: "idle" | AcmeJobStatus;
  latestJobTrigger?: AcmeJobTrigger;
  latestJobAt?: string;
  lastError?: string;
};

export type AcmeApplicationOverviewItem = {
  id: string;
  name?: string;
  primaryDomain: string;
  domains: string[];
  dnsType: string;
  providerLabel: string;
  renewEnabled: boolean;
  createdAt: string;
  updatedAt: string;
  latestJob?: {
    id: string;
    status: "idle" | AcmeJobStatus;
    trigger: AcmeJobTrigger;
    createdAt: string;
    message?: string;
  } | null;
  certificate?: {
    exists: boolean;
    validFrom?: string;
    validTo?: string;
    dnsNames?: string[];
    issuer?: string;
  } | null;
  library?: {
    linked: boolean;
    certificateId?: string;
    isActive?: boolean;
  } | null;
};

export type AcmeOverview = {
  acmeState: {
    status: "uninstalled" | "installing" | "installed" | "error";
    progress: number;
    message: string;
  };
  clientSettings: {
    certificateAuthority: AcmeCertificateAuthority;
    updatedAt: string;
  };
  lock: {
    locked: boolean;
    jobId?: string;
    applicationId?: string;
    reason?: AcmeJobTrigger;
    startedAt?: string;
  };
  applications: AcmeApplicationOverviewItem[];
  runningJob?: {
    id: string;
    applicationId?: string;
    status: AcmeJobStatus;
    progress: number;
  } | null;
};

export type AcmeApplicationPayload = {
  name?: string;
  domains: string[];
  dnsType: string;
  credentials?: Record<string, string>;
  renewEnabled?: boolean;
  submitNow?: boolean;
};

export const AcmeAPI = {
  async updateClientSettings(payload: {
    certificateAuthority: AcmeCertificateAuthority;
  }): Promise<{
    certificateAuthority: AcmeCertificateAuthority;
    updatedAt: string;
    synced: boolean;
    accountEmail?: string;
  }> {
    const res = await apiClient.post("/acme/client-settings", payload);
    return res.data.data;
  },
  async getSubdomainRecommendation(): Promise<{
    mode: "wildcard_parent" | "single_host" | "manual";
    root_domain?: string;
    auth_host?: string;
    recommended_domains: string[];
    covered_hosts: string[];
    uncovered_hosts: string[];
    warnings: string[];
    can_autofill: boolean;
    summary: string;
  }> {
    const res = await apiClient.get("/acme/subdomain-recommendation");
    return res.data.data;
  },
  async dnsProviders(): Promise<AcmeDnsProvider[]> {
    const res = await apiClient.get("/acme/dns-providers");
    return res.data.data || [];
  },
  async overview(): Promise<AcmeOverview> {
    const res = await apiClient.get("/acme/overview");
    return res.data.data;
  },
  async status(): Promise<{
    status: "uninstalled" | "installing" | "installed" | "error";
    progress: number;
    message: string;
    certificateAuthority: AcmeCertificateAuthority;
    certificateAuthorityUpdatedAt?: string;
    acmeCert?: { primaryDomain: string; info: any } | null;
  }> {
    const res = await apiClient.get("/acme/status");
    return res.data.data;
  },
  async getConfig(): Promise<{
    domains: string[];
    dnsType: string;
    credentials: Record<string, string>;
    updatedAt: string;
  } | null> {
    const res = await apiClient.get("/acme/config");
    return res.data.data || null;
  },
  async saveConfig(payload: {
    domains: string[];
    dnsType: string;
    credentials?: Record<string, string>;
  }): Promise<{
    domains: string[];
    dnsType: string;
    credentials: Record<string, string>;
    updatedAt: string;
  }> {
    const res = await apiClient.post("/acme/config", payload);
    return res.data.data;
  },
  async init(): Promise<void> {
    await apiClient.post("/acme/init");
  },
  async uninstall(): Promise<void> {
    await apiClient.delete("/acme");
  },
  async getApplications(): Promise<AcmeApplicationRecord[]> {
    const res = await apiClient.get("/acme/applications");
    return res.data.data || [];
  },
  async getApplication(id: string): Promise<AcmeApplicationRecord> {
    const res = await apiClient.get(
      `/acme/applications/${encodeURIComponent(id)}`,
    );
    return res.data.data;
  },
  async createApplication(payload: AcmeApplicationPayload): Promise<{
    application: AcmeApplicationRecord;
    job?: AcmeJobData;
    lock?: AcmeOverview["lock"];
  }> {
    const res = await apiClient.post("/acme/applications", payload);
    return res.data.data;
  },
  async updateApplication(
    id: string,
    payload: AcmeApplicationPayload,
  ): Promise<{
    application: AcmeApplicationRecord;
    job?: AcmeJobData;
    lock?: AcmeOverview["lock"];
  }> {
    const res = await apiClient.patch(
      `/acme/applications/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data.data;
  },
  async requestApplication(id: string): Promise<{
    job: AcmeJobData;
    lock: AcmeOverview["lock"];
  }> {
    const res = await apiClient.post(
      `/acme/applications/${encodeURIComponent(id)}/request`,
    );
    return res.data.data;
  },
  async deleteApplication(id: string): Promise<void> {
    await apiClient.delete(`/acme/applications/${encodeURIComponent(id)}`);
  },
  async deleteApplicationCertificate(id: string): Promise<void> {
    await apiClient.delete(
      `/acme/applications/${encodeURIComponent(id)}/certificate`,
    );
  },
  async syncApplicationLibrary(
    id: string,
  ): Promise<{ certificateId: string; linked: boolean }> {
    const res = await apiClient.post(
      `/acme/applications/${encodeURIComponent(id)}/library/sync`,
    );
    return res.data.data;
  },
  async deployApplication(id: string): Promise<void> {
    await apiClient.post(`/acme/applications/${encodeURIComponent(id)}/deploy`);
  },
  async request(payload: {
    domains: string[];
    dnsType: string;
    credentials?: Record<string, string>;
  }): Promise<{ jobId: string }> {
    const res = await apiClient.post("/acme/request", payload);
    return res.data.data;
  },
  async stopActiveJob(): Promise<{
    stopped: boolean;
    job: AcmeJobData | null;
    processResult: {
      matchedPids: number[];
      remainingPids: number[];
      errors: string[];
    };
  }> {
    const res = await apiClient.post("/acme/jobs/active/stop");
    return res.data.data;
  },
  async job(id: string): Promise<AcmeJobData> {
    const res = await apiClient.get(`/acme/jobs/${encodeURIComponent(id)}`);
    return res.data.data;
  },
  async logs(id: string): Promise<string[]> {
    const res = await apiClient.get(
      `/acme/jobs/${encodeURIComponent(id)}/logs`,
    );
    return res.data.data || [];
  },
  async poll(
    id: string,
    opts?: { limit?: number; order?: "asc" | "desc" },
  ): Promise<{
    job: AcmeJobData;
    logs: string[];
    analysis?: AcmeLogAnalysis | null;
  }> {
    const res = await apiClient.get(
      `/acme/jobs/${encodeURIComponent(id)}/poll`,
      { params: { limit: opts?.limit, order: opts?.order } },
    );
    return res.data.data;
  },
  async certInfo(domain: string): Promise<{ domain: string; info: any }> {
    const res = await apiClient.get(
      `/acme/certs/${encodeURIComponent(domain)}`,
    );
    return res.data.data;
  },
  async download(domain: string): Promise<Blob> {
    const res = await apiClient.get(
      `/acme/certs/${encodeURIComponent(domain)}/download`,
      { responseType: "blob" },
    );
    return res.data;
  },
  async deploy(domain: string): Promise<void> {
    await apiClient.post(`/acme/certs/${encodeURIComponent(domain)}/deploy`);
  },
  async deleteCert(domain: string): Promise<void> {
    await apiClient.delete(`/acme/certs/${encodeURIComponent(domain)}`);
  },
};

export const DashboardAPI = {
  async getStats(
    rangeSec: number,
    userIdOrOptions?: string | { userId?: string; host?: string },
  ): Promise<DashboardStats> {
    const options =
      typeof userIdOrOptions === "string"
        ? { userId: userIdOrOptions }
        : (userIdOrOptions ?? {});
    const res = await apiClient.get("/dashboard/stats", {
      params: { rangeSec, ...options },
    });
    return res.data.data;
  },
  async getRealtime(): Promise<TrafficStats> {
    const res = await apiClient.get("/dashboard/realtime");
    return res.data.data;
  },
  async getHostActiveIps(host: string): Promise<HostActiveIpsPayload> {
    const res = await apiClient.get("/dashboard/active-ips", {
      params: { host },
    });
    return res.data.data;
  },
};

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

export type DDNSLogEntry = {
  time: string;
  level: "info" | "error" | "warn";
  message: string;
};

export type DDNSIpSource = "public" | "interface" | "static" | "domain";
export type DDNSUpdateScope = "dual_stack" | "ipv6_only" | "ipv4_only";

export type DDNSStatusPayload = {
  enabled: boolean;
  provider: string | null;
  updateIntervalMinutes: number;
  updateScope: DDNSUpdateScope;
  ipSource: DDNSIpSource;
  networkInterface: string;
  lastIP: {
    ipv4: string | null;
    ipv6: string | null;
    updated_at: string | null;
  };
  lastCheck: {
    checked_at: string | null;
    outcome: "updated" | "noop" | "skipped" | "error" | null;
    message: string | null;
  };
  primaryTargetId: string | null;
  extraTargetCount: number;
  enabledExtraTargetCount: number;
  targets: DDNSTargetSummaryPayload[];
};

export type DDNSSettingsPayload = {
  updateIntervalMinutes: number;
};

export type DDNSTargetSummaryPayload = {
  id: string;
  name: string;
  isPrimary: boolean;
  enabled: boolean;
  provider: string | null;
  updateScope: DDNSUpdateScope;
  providerLabel: string;
  domainSummary: string;
  createdAt: string;
  updatedAt: string;
  sortOrder: number;
  lastIP: {
    ipv4: string | null;
    ipv6: string | null;
    updated_at: string | null;
  };
  lastCheck: {
    checked_at: string | null;
    outcome: "updated" | "noop" | "skipped" | "error" | null;
    message: string | null;
  };
};

export type DDNSTargetDetailPayload = DDNSTargetSummaryPayload & {
  rawName?: string;
  config: Record<string, string>;
};

export type DDNSTargetListPayload = {
  primaryTargetId: string | null;
  total: number;
  extraCount: number;
  enabledExtraCount: number;
  items: DDNSTargetSummaryPayload[];
};

export type DDNSNetworkInterfacePayload = {
  name: string;
  label: string;
  summary: string;
  hasIpv4: boolean;
  hasIpv6: boolean;
  addresses: Array<{
    family: "ipv4" | "ipv6";
    address: string;
    cidr: string | null;
    internal: boolean;
    source?: "runtime" | "docker_host";
  }>;
  selectableAddresses: Array<{
    family: "ipv4" | "ipv6";
    address: string;
    cidr: string | null;
    internal: boolean;
    source?: "runtime" | "docker_host";
  }>;
  source?: "runtime" | "docker_host";
};

export type DDNSPollPayload = {
  cursor: number;
  reset: boolean;
  logs: DDNSLogEntry[];
  status: DDNSStatusPayload;
};

export type FrpcTcpItem = {
  name: string;
  type: string;
  status: string;
  err: string;
  local_addr: string;
  plugin: string;
  remote_addr: string;
};

export type FrpcInstanceSummary = {
  serverAddr: string;
  serverPort: string;
  localPort: string;
  remotePort: string;
};

export type FrpcInstanceStatus = {
  id: string;
  name: string;
  isPrimary: boolean;
  configPath: string;
  workDir: string;
  createdAt: string;
  updatedAt: string;
  sortOrder: number;
  desiredRunning: boolean;
  running: boolean;
  attached: boolean;
  pid: number | null;
  startedAt: string | null;
  stoppedAt: string | null;
  lastExitCode: number | null;
  lastMessage: string | null;
  summary: FrpcInstanceSummary;
};

export type FrpcInstancesOverview = {
  initialized: boolean;
  platform: string;
  primaryInstanceId: string;
  total: number;
  extraCount: number;
  runningCount: number;
  defaults: { local_port: string };
  items: FrpcInstanceStatus[];
};

export type FrpcInstanceDetail = {
  item: FrpcInstanceStatus;
  content: string;
  logs: string[];
};

export type FrpcStatusPayload = FrpcInstanceStatus & {
  tcp: FrpcTcpItem[];
  instances?: FrpcInstancesOverview;
};

export type FrpcPollPayload = {
  cursor: number;
  reset: boolean;
  logs: string[];
  status: FrpcStatusPayload;
};

export type FrpcInstancePollPayload = {
  cursor: number;
  reset: boolean;
  logs: string[];
  status: FrpcInstanceStatus;
};

export type CloudflaredStatusPayload = {
  running: boolean;
  pid: number | null;
};

export type CloudflaredPollPayload = {
  cursor: number;
  reset: boolean;
  logs: string[];
  status: CloudflaredStatusPayload;
};

export const DDNSAPI = {
  async getStatus(): Promise<DDNSStatusPayload> {
    const res = await apiClient.get("/ddns/status");
    return res.data.data;
  },
  async toggle(enabled: boolean): Promise<void> {
    await apiClient.post("/ddns/toggle", { enabled });
  },
  async getSettings(): Promise<DDNSSettingsPayload> {
    const res = await apiClient.get("/ddns/settings");
    return res.data.data;
  },
  async saveSettings(payload: {
    updateIntervalMinutes: number;
  }): Promise<DDNSSettingsPayload> {
    const res = await apiClient.post("/ddns/settings", payload);
    return res.data.data;
  },
  async getProviders(): Promise<
    Array<{
      name: string;
      label: string;
      fields: Array<{
        key: string;
        label: string;
        type: string;
        placeholder?: string;
        required?: boolean;
        options?: Array<{ label: string; value: string }>;
        description?: string;
      }>;
      capabilities?: {
        addressMode?: "dual_stack" | "single_address";
        ipSources?: DDNSIpSource[];
      };
    }>
  > {
    const res = await apiClient.get("/ddns/providers");
    return res.data.data;
  },
  async getNetworkInterfaces(): Promise<DDNSNetworkInterfacePayload[]> {
    const res = await apiClient.get("/ddns/interfaces");
    return res.data.data;
  },
  async setProvider(provider: string): Promise<void> {
    await apiClient.post("/ddns/provider", { provider });
  },
  async getConfig(provider: string): Promise<Record<string, string>> {
    const res = await apiClient.get(
      `/ddns/config/${encodeURIComponent(provider)}`,
    );
    return res.data.data;
  },
  async saveConfig(
    provider: string,
    config: Record<string, string>,
  ): Promise<void> {
    await apiClient.post(`/ddns/config/${encodeURIComponent(provider)}`, {
      config,
    });
  },
  async test(): Promise<{
    success: boolean;
    message: string;
    data?: {
      ipv4: string | null;
      ipv6: string | null;
      source?: DDNSIpSource;
      sourceLabel?: string;
    };
  }> {
    const res = await apiClient.post("/ddns/test");
    return res.data;
  },
  async getTargets(): Promise<DDNSTargetListPayload> {
    const res = await apiClient.get("/ddns/targets");
    return res.data.data;
  },
  async getTarget(id: string): Promise<DDNSTargetDetailPayload> {
    const res = await apiClient.get(`/ddns/targets/${encodeURIComponent(id)}`);
    return res.data.data;
  },
  async createTarget(payload: {
    name?: string;
    provider: string;
    enabled?: boolean;
    config: Record<string, string>;
  }): Promise<DDNSTargetDetailPayload> {
    const res = await apiClient.post("/ddns/targets", payload);
    return res.data.data;
  },
  async updateTarget(
    id: string,
    payload: {
      name?: string;
      provider: string;
      enabled?: boolean;
      config: Record<string, string>;
    },
  ): Promise<DDNSTargetDetailPayload> {
    const res = await apiClient.put(
      `/ddns/targets/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data.data;
  },
  async deleteTarget(id: string): Promise<void> {
    await apiClient.delete(`/ddns/targets/${encodeURIComponent(id)}`);
  },
  async setTargetEnabled(id: string, enabled: boolean): Promise<void> {
    await apiClient.post(`/ddns/targets/${encodeURIComponent(id)}/enabled`, {
      enabled,
    });
  },
  async testTarget(id: string): Promise<{
    success: boolean;
    message: string;
    data?: {
      ipv4: string | null;
      ipv6: string | null;
      source?: DDNSIpSource;
      sourceLabel?: string;
    };
  }> {
    const res = await apiClient.post(
      `/ddns/targets/${encodeURIComponent(id)}/test`,
    );
    return res.data;
  },
  async getLogs(limit = 200): Promise<DDNSLogEntry[]> {
    const res = await apiClient.get("/ddns/logs", { params: { limit } });
    return res.data.data;
  },
  async clearLogs(): Promise<void> {
    await apiClient.delete("/ddns/logs");
  },
  async poll(cursor?: number): Promise<DDNSPollPayload> {
    const res = await apiClient.get("/ddns/poll", {
      params: typeof cursor === "number" ? { cursor } : undefined,
    });
    return res.data.data;
  },
};

export const CidrAPI = {
  async getProvinces(): Promise<CidrProvincesPayload> {
    const res = await apiClient.get("/cidr/provinces");
    return res.data.data;
  },
  async getCities(province: string): Promise<CidrCitiesPayload> {
    const res = await apiClient.get("/cidr/cities", {
      params: { province },
    });
    return res.data.data;
  },
  async getSelector(province?: string): Promise<CidrSelectorPayload> {
    const res = await apiClient.get("/cidr/selector", {
      params: province ? { province } : undefined,
    });
    return res.data.data;
  },
  async getCidrs(payload: {
    province: string;
    city?: string | null;
  }): Promise<CidrLookupPayload> {
    const params: Record<string, string> = {
      province: payload.province,
    };
    if (payload.city) {
      params.city = payload.city;
    }
    const res = await apiClient.get("/cidr/cidrs", { params });
    return res.data.data;
  },
};
