import { apiClient } from "./client";

export type { SSLConfig, SSLSharedFilesPayload, SSLStatus } from "../../types";

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
  async resourceStatus(): Promise<{
    supported: boolean;
    initialized: boolean;
    platform: string;
    installedVersion?: string;
    availableVersion?: string;
    progress: { status: "idle" | "downloading" | "verifying" | "completed" | "cancelled" | "error"; percent: number; error?: string };
    providerIds: string[];
  }> {
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
  async updateClientSettings(payload: {
    certificateAuthority: AcmeCertificateAuthority;
    accountEmail?: string;
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
