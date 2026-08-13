import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";
import type {
  AuthCredentialSettings,
  DashboardDisplayConfig,
  DockerAdminBootstrapState,
  LocaleConfig,
  WOLFeatureConfig,
} from "../../types";
import { apiClient } from "./client";
import {
  HOST_MAPPING_CATALOG_REVISION_HEADER,
  hostMappingsRevisionFromHeaders,
  type RevisionedConfig,
} from "./config-revisions";

type AuthCredentialSettingsUpdate =
  ApiContractComponents["schemas"]["AuthCredentialSettingsUpdateData"];
type DashboardDisplayUpdate =
  ApiContractComponents["schemas"]["DashboardDisplayUpdateData"];
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
type DefaultTunnelUpdate =
  ApiContractComponents["schemas"]["DefaultTunnelUpdateData"];
type WOLFeatureConfigUpdate =
  ApiContractComponents["schemas"]["WolFeatureConfigUpdateData"];
type PanelPasswordRequest =
  ApiContractComponents["schemas"]["PanelPasswordBodyData"];
type PanelLoginRequest = ApiContractComponents["schemas"]["PanelLoginBodyData"];
type SyncRoutesResponse =
  ApiContractOperations["post_api_admin_sync_routes"]["responses"][200]["content"]["application/json"];

export const configCoreApi = {
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

  // Sync routes.
  async syncRoutes(): Promise<SyncRoutesResponse> {
    const res = await apiClient.post("/sync-routes");
    return res.data;
  },
};
