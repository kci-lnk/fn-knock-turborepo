import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";
import { apiClient } from "./client";

export type { SSLConfig, SSLSharedFilesPayload, SSLStatus } from "../../types";

type AcmeSchemas = ApiContractComponents["schemas"];

export type AcmeCertificateAuthority =
  AcmeSchemas["AcmeClientSettingsData"]["certificateAuthority"];
export type AcmeJobStatus = AcmeSchemas["AcmeJobData"]["status"];
export type AcmeJobTrigger = NonNullable<AcmeSchemas["AcmeJobData"]["trigger"]>;
export type AcmeDnsProvider = AcmeSchemas["AcmeDnsProviderData"];
export type AcmeLogAnalysis = AcmeSchemas["AcmeLogAnalysisData"];
export type AcmeJobData = AcmeSchemas["AcmeJobData"];
export type AcmeApplicationRecord = AcmeSchemas["AcmeApplicationData"];
export type AcmeApplicationOverviewItem =
  AcmeSchemas["AcmeApplicationOverviewData"];
export type AcmeOverview = AcmeSchemas["AcmeOverviewData"];
export type AcmeApplicationPayload = AcmeSchemas["AcmeApplicationBodyData"];

type AcmeResourceStatus = AcmeSchemas["AcmeResourceStatusData"];
type AcmeClientSettingsBody = AcmeSchemas["AcmeClientSettingsBodyData"];
type AcmeClientSettingsUpdate = AcmeSchemas["AcmeClientSettingsUpdateData"];
type AcmeSubdomainRecommendation =
  AcmeSchemas["AcmeSubdomainRecommendationData"];
type AcmeStatus = AcmeSchemas["AcmeStatusData"];
type AcmeConfig = AcmeSchemas["AcmeConfigData"];
type AcmeConfigBody = AcmeSchemas["AcmeConfigBodyData"];
type AcmeApplicationMutation = AcmeSchemas["AcmeApplicationMutationData"];
type AcmeApplicationRequest = AcmeSchemas["AcmeApplicationRequestData"];
type AcmeLibrarySync = AcmeSchemas["AcmeLibrarySyncData"];
type AcmeLegacyRequestBody = AcmeSchemas["AcmeLegacyRequestBodyData"];
type AcmeLegacyRequest = AcmeSchemas["AcmeLegacyRequestData"];
type AcmeStopJob = AcmeSchemas["AcmeStopJobData"];
type AcmeJobPoll = AcmeSchemas["AcmeJobPollData"];
type AcmeCertificate = AcmeSchemas["AcmeCertificateData"];
type AcmePollQuery = NonNullable<
  ApiContractOperations["get_api_admin_acme_jobs__id__poll"]["parameters"]["query"]
>;

export const AcmeAPI = {
  async resourceStatus(): Promise<AcmeResourceStatus> {
    const res = await apiClient.get("/acme/resource/status");
    return res.data.data;
  },
  async initializeResource(): Promise<void> {
    await apiClient.post("/acme/resource/initialize");
  },
  async cancelResourceInitialization(): Promise<void> {
    await apiClient.post("/acme/resource/cancel");
  },
  async deleteResource(): Promise<void> {
    await apiClient.delete("/acme/resource");
  },
  async updateClientSettings(
    payload: AcmeClientSettingsBody,
  ): Promise<AcmeClientSettingsUpdate> {
    const body = payload satisfies AcmeClientSettingsBody;
    const res = await apiClient.post("/acme/client-settings", body);
    return res.data.data;
  },
  async getSubdomainRecommendation(): Promise<AcmeSubdomainRecommendation> {
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
  async status(signal?: AbortSignal): Promise<AcmeStatus> {
    const res = await apiClient.get("/acme/status", { signal });
    return res.data.data;
  },
  async getConfig(): Promise<AcmeConfig | null> {
    const res = await apiClient.get("/acme/config");
    return res.data.data || null;
  },
  async saveConfig(payload: AcmeConfigBody): Promise<AcmeConfig> {
    const body = payload satisfies AcmeConfigBody;
    const res = await apiClient.post("/acme/config", body);
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
  async createApplication(
    payload: AcmeApplicationPayload,
  ): Promise<AcmeApplicationMutation> {
    const body = payload satisfies AcmeApplicationPayload;
    const res = await apiClient.post("/acme/applications", body);
    return res.data.data;
  },
  async updateApplication(
    id: string,
    payload: AcmeApplicationPayload,
  ): Promise<AcmeApplicationMutation> {
    const body = payload satisfies AcmeApplicationPayload;
    const res = await apiClient.patch(
      `/acme/applications/${encodeURIComponent(id)}`,
      body,
    );
    return res.data.data;
  },
  async requestApplication(id: string): Promise<AcmeApplicationRequest> {
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
  async syncApplicationLibrary(id: string): Promise<AcmeLibrarySync> {
    const res = await apiClient.post(
      `/acme/applications/${encodeURIComponent(id)}/library/sync`,
    );
    return res.data.data;
  },
  async deployApplication(id: string): Promise<void> {
    await apiClient.post(`/acme/applications/${encodeURIComponent(id)}/deploy`);
  },
  async request(payload: AcmeLegacyRequestBody): Promise<AcmeLegacyRequest> {
    const body = payload satisfies AcmeLegacyRequestBody;
    const res = await apiClient.post("/acme/request", body);
    return res.data.data;
  },
  async stopActiveJob(): Promise<AcmeStopJob> {
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
    opts?: {
      limit?: number;
      order?: "asc" | "desc";
      signal?: AbortSignal;
    },
  ): Promise<AcmeJobPoll> {
    const params = {
      limit: opts?.limit,
      order: opts?.order,
    } satisfies AcmePollQuery;
    const res = await apiClient.get(
      `/acme/jobs/${encodeURIComponent(id)}/poll`,
      { params, signal: opts?.signal },
    );
    return res.data.data;
  },
  async certInfo(domain: string): Promise<AcmeCertificate> {
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
