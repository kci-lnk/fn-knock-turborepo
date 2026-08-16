import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";
import type {
  AdvancedAuthConfig,
  GatewayHostResponseDetails,
  GatewayProxyHeadersDetails,
  GatewaySettings,
  GatewayVisibilityDetails,
  HostMapping,
  HostMappingBasicAuth,
  HostMappingGroup,
  HostMappingRefreshSummary,
  ProxyMapping,
  SSLConfig,
  ExternalCertificateBinding,
  ExternalCertificateBindingCredential,
  SSLSharedFilesPayload,
  SSLStatus,
  SubdomainModeConfig,
  UrlMetadataPreview,
} from "../../types";
import { apiClient } from "./client";
import {
  toHostMappingBasicAuthPayload,
  toHostMappingUpdatePayload,
} from "./host-mapping-payload";
import {
  HOST_MAPPING_CATALOG_REVISION_HEADER,
  hostMappingsRevisionFromHeaders,
  type RevisionedHostMappingCatalog,
  type RevisionedHostMappings,
} from "./config-revisions";

type ProxyProtocolForceContract =
  ApiContractComponents["schemas"]["ProxyProtocolForceData"];
type ProxyMappingsUpdate =
  ApiContractComponents["schemas"]["ProxyMappingsUpdateData"];
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
type GatewayVisibilityUpdate =
  ApiContractComponents["schemas"]["GatewayVisibilityUpdateData"];
type GatewayProxyHeadersUpdate =
  ApiContractComponents["schemas"]["GatewayProxyHeadersUpdateData"];
type GatewayHostResponseUpdate =
  ApiContractComponents["schemas"]["GatewayHostResponseUpdateData"];
type GatewaySettingsUpdate =
  ApiContractComponents["schemas"]["GatewaySettingsUpdateData"];
type DefaultRouteUpdate =
  ApiContractComponents["schemas"]["DefaultRouteUpdateData"];
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
type ExternalCertificateBindingCreateBody =
  ApiContractComponents["schemas"]["ExternalCertificateBindingCreateBodyData"];
type ExternalCertificateBindingUpdateBody =
  ApiContractComponents["schemas"]["ExternalCertificateBindingUpdateBodyData"];

export type AdvancedAuthDetails =
  ApiContractComponents["schemas"]["AdvancedAuthDetailsData"];
export type HostMappingBasicAuthProbeResult =
  ApiContractComponents["schemas"]["HostMappingBasicAuthProbeData"];
export const configProxyApi = {
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
  async getExternalCertificateBindings(): Promise<
    ExternalCertificateBinding[]
  > {
    const res = await apiClient.get("/ssl/external-bindings");
    return res.data.data || [];
  },
  async createExternalCertificateBinding(
    name: string,
    provider: ExternalCertificateBinding["provider"],
  ): Promise<ExternalCertificateBindingCredential> {
    const payload = {
      name,
      provider,
    } satisfies ExternalCertificateBindingCreateBody;
    const res = await apiClient.post("/ssl/external-bindings", payload);
    return res.data.data;
  },
  async updateExternalCertificateBinding(
    id: string,
    update: ExternalCertificateBindingUpdateBody,
  ): Promise<ExternalCertificateBinding> {
    const payload = update satisfies ExternalCertificateBindingUpdateBody;
    const res = await apiClient.patch(
      `/ssl/external-bindings/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data.data;
  },
  async rotateExternalCertificateBindingToken(
    id: string,
  ): Promise<ExternalCertificateBindingCredential> {
    const res = await apiClient.post(
      `/ssl/external-bindings/${encodeURIComponent(id)}/rotate-token`,
    );
    return res.data.data;
  },
  async deleteExternalCertificateBinding(id: string): Promise<void> {
    await apiClient.delete(`/ssl/external-bindings/${encodeURIComponent(id)}`);
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
};
