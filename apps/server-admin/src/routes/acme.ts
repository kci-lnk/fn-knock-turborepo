import { Elysia, t } from "elysia";
import { acmePlugin } from "../plugins/acme";
import {
  configManager,
  type AcmeApplication,
  type AcmeApplicationSaveResult,
} from "../lib/redis";
import { syncSSLDeploymentToGateway } from "../lib/ssl-gateway";
import { DEFAULT_REDIS_LOG_BUFFER_MAX_LEN } from "../lib/redis-log-buffer";
import { buildSubdomainCertificateRecommendation } from "../lib/subdomain-mode";
import {
  failReservedAcmeApplicationJob,
  reserveAcmeApplicationJob,
  runReservedAcmeApplicationJob,
  startAcmeApplicationJob,
} from "../lib/acme-job-runner";

type DnsProvider = {
  dnsType: string;
  label: string;
  group: string;
  credentialSchemes: DnsCredentialScheme[];
};

type DnsCredentialField = {
  key: string;
  label?: string;
  description?: string;
  required?: boolean;
};

type DnsCredentialScheme = {
  id: string;
  label: string;
  description?: string;
  fields: DnsCredentialField[];
};

const createCredentialScheme = (
  id: string,
  label: string,
  fieldKeys: string[],
  options?: {
    description?: string;
    optionalKeys?: string[];
    fields?: Partial<Record<string, Omit<DnsCredentialField, "key">>>;
  },
): DnsCredentialScheme => {
  const optionalKeys = new Set(options?.optionalKeys || []);

  return {
    id,
    label,
    description: options?.description,
    fields: fieldKeys.map((key) => ({
      key,
      required: !optionalKeys.has(key),
      ...(options?.fields?.[key] || {}),
    })),
  };
};

const createSingleSchemeProvider = (
  dnsType: string,
  label: string,
  group: string,
  envKeys: string[],
): DnsProvider => ({
  dnsType,
  label,
  group,
  credentialSchemes: [createCredentialScheme("default", "默认凭据", envKeys)],
});

const getProviderAllCredentialKeys = (provider: DnsProvider) => {
  const keys: string[] = [];
  const seen = new Set<string>();
  for (const scheme of provider.credentialSchemes) {
    for (const field of scheme.fields) {
      if (seen.has(field.key)) continue;
      seen.add(field.key);
      keys.push(field.key);
    }
  }
  return keys;
};

const getSatisfiedCredentialScheme = (
  provider: DnsProvider,
  credentials: Record<string, string>,
) => {
  return (
    provider.credentialSchemes.find((scheme) =>
      scheme.fields
        .filter((field) => field.required !== false)
        .every((field) => Boolean(credentials[field.key])),
    ) || null
  );
};

const formatCredentialRequirements = (provider: DnsProvider) => {
  if (provider.credentialSchemes.length === 1) {
    const requiredKeys = provider.credentialSchemes[0]!.fields.filter(
      (field) => field.required !== false,
    ).map((field) => field.key);
    return requiredKeys.join(", ");
  }

  return provider.credentialSchemes
    .map((scheme) => {
      const requiredKeys = scheme.fields
        .filter((field) => field.required !== false)
        .map((field) => field.key)
        .join(", ");
      const optionalKeys = scheme.fields
        .filter((field) => field.required === false)
        .map((field) => field.key);
      const suffix = optionalKeys.length
        ? `；可选 ${optionalKeys.join(", ")}`
        : "";
      return `${scheme.label}: ${requiredKeys}${suffix}`;
    })
    .join("；或 ");
};

const dnsProviders: DnsProvider[] = [
  {
    dnsType: "dns_cf",
    label: "Cloudflare",
    group: "常用",
    credentialSchemes: [
      createCredentialScheme(
        "global-key",
        "Global API Key",
        ["CF_Key", "CF_Email"],
        {
          description: "兼容 Cloudflare 旧版 Global API Key 方式。",
          fields: {
            CF_Key: { label: "Global API Key" },
            CF_Email: { label: "账户邮箱" },
          },
        },
      ),
      createCredentialScheme(
        "api-token",
        "API Token",
        ["CF_Token", "CF_Zone_ID", "CF_Account_ID"],
        {
          description:
            "推荐。仅需填写 Token；如已知 Zone ID 或 Account ID，可一并填写以减少自动探测。",
          optionalKeys: ["CF_Zone_ID", "CF_Account_ID"],
          fields: {
            CF_Token: { label: "API Token" },
            CF_Zone_ID: { label: "Zone ID" },
            CF_Account_ID: { label: "Account ID" },
          },
        },
      ),
    ],
  },
  createSingleSchemeProvider("dns_ali", "阿里云 DNS", "常用", [
    "Ali_Key",
    "Ali_Secret",
  ]),
  createSingleSchemeProvider("dns_dp", "DNSPod", "常用", ["DP_Id", "DP_Key"]),
  createSingleSchemeProvider(
    "dns_tencent",
    "腾讯云 DNSPod (TencentCloud)",
    "常用",
    ["Tencent_SecretId", "Tencent_SecretKey"],
  ),
  createSingleSchemeProvider("dns_gd", "GoDaddy", "常用", [
    "GD_Key",
    "GD_Secret",
  ]),
  createSingleSchemeProvider("dns_dgon", "DigitalOcean", "常用", [
    "DO_API_KEY",
  ]),
  createSingleSchemeProvider("dns_netlify", "Netlify", "常用", [
    "NETLIFY_TOKEN",
  ]),
  createSingleSchemeProvider("dns_vercel", "Vercel", "常用", [
    "VERCEL_TOKEN",
    "VERCEL_TEAM_ID",
  ]),
  createSingleSchemeProvider("dns_aws", "AWS Route53", "常用", [
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_REGION",
  ]),
  createSingleSchemeProvider("dns_google", "Google Cloud DNS", "常用", [
    "GCE_PROJECT",
    "GCE_SERVICE_ACCOUNT_FILE",
  ]),
  createSingleSchemeProvider("dns_azure", "Azure DNS", "常用", [
    "AZUREDNS_SUBSCRIPTIONID",
    "AZUREDNS_TENANTID",
    "AZUREDNS_APPID",
    "AZUREDNS_CLIENTSECRET",
  ]),
  createSingleSchemeProvider("dns_linode_v4", "Linode", "国际", [
    "LINODE_V4_API_KEY",
  ]),
  createSingleSchemeProvider("dns_vultr", "Vultr", "国际", ["VULTR_API_KEY"]),
  createSingleSchemeProvider("dns_ovh", "OVH", "国际", [
    "OVH_AK",
    "OVH_AS",
    "OVH_CK",
  ]),
  createSingleSchemeProvider("dns_hetzner", "Hetzner", "国际", [
    "HETZNER_Token",
  ]),
  createSingleSchemeProvider("dns_namecheap", "Namecheap", "国际", [
    "NAMECHEAP_API_KEY",
    "NAMECHEAP_USERNAME",
    "NAMECHEAP_SOURCEIP",
  ]),
  createSingleSchemeProvider("dns_porkbun", "Porkbun", "国际", [
    "PORKBUN_API_KEY",
    "PORKBUN_SECRET_API_KEY",
  ]),
  createSingleSchemeProvider("dns_dynv6", "dynv6", "国际", ["DYNV6_TOKEN"]),
  createSingleSchemeProvider("dns_cloudns", "ClouDNS", "国际", [
    "CLOUDNS_AUTH_ID",
    "CLOUDNS_AUTH_PASSWORD",
  ]),
  createSingleSchemeProvider("dns_gandi_livedns", "Gandi LiveDNS", "国际", [
    "GANDI_LIVEDNS_KEY",
  ]),
  createSingleSchemeProvider("dns_nsone", "NS1", "国际", ["NS1_Key"]),
  createSingleSchemeProvider("dns_dnsimple", "DNSimple", "国际", [
    "DNSimple_OAUTH_TOKEN",
    "DNSimple_ACCOUNT_ID",
  ]),
  createSingleSchemeProvider("dns_he", "Hurricane Electric", "国际", [
    "HE_Username",
    "HE_Password",
  ]),
  createSingleSchemeProvider("dns_transip", "TransIP", "国际", [
    "TRANSIP_Username",
    "TRANSIP_Key_File",
  ]),
];

const normalizeDnsType = (value: string | undefined | null) => {
  if (!value) return null;
  const v = value.trim();
  if (!v) return null;
  if (v === "aliyun") return "dns_ali";
  if (v === "cloudflare") return "dns_cf";
  if (v === "dnspod") return "dns_dp";
  if (/^dns_[a-z0-9_]+$/i.test(v)) return v;
  return null;
};

const normalizeDomains = (domains: string[]) => {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const raw of domains || []) {
    const v = String(raw ?? "")
      .trim()
      .toLowerCase();
    if (!v) continue;
    if (!isValidDomain(v)) continue;
    if (seen.has(v)) continue;
    seen.add(v);
    out.push(v);
  }
  return out;
};

const isValidDomain = (value: string) => {
  if (!value) return false;
  if (value.length > 253) return false;
  const v = value.trim();
  if (!v) return false;
  if (v.includes("..")) return false;
  if (v.startsWith(".") || v.endsWith(".")) return false;
  if (v.includes("/") || v.includes(" ") || v.includes("\t")) return false;
  return /^(\*\.)?([a-z0-9-]+\.)+[a-z0-9-]+$/i.test(v);
};

const normalizeCredentials = (value: unknown) => {
  const out: Record<string, string> = {};
  if (!value || typeof value !== "object") return out;
  for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
    const kk = String(k ?? "").trim();
    const vv = String(v ?? "").trim();
    if (!kk || !vv) continue;
    out[kk] = vv;
  }
  return out;
};

const validateAndNormalizeAcmeRequest = (input: {
  domains: string[];
  dnsType?: string;
  provider?: string;
  credentials?: Record<string, string>;
}) => {
  const domains = normalizeDomains(input.domains);
  if (domains.length === 0) throw new Error("域名列表不能为空或格式无效");

  const dnsType = normalizeDnsType(input.dnsType ?? input.provider);
  if (!dnsType) throw new Error("dnsType required");
  const provider = dnsProviders.find((p) => p.dnsType === dnsType) || null;
  if (!provider) throw new Error("不支持的 DNS 服务商");

  const allowedCredentialKeys = new Set(getProviderAllCredentialKeys(provider));
  const credentials = Object.fromEntries(
    Object.entries(normalizeCredentials(input.credentials)).filter(([key]) =>
      allowedCredentialKeys.has(key),
    ),
  );
  const matchedScheme = getSatisfiedCredentialScheme(provider, credentials);
  if (!matchedScheme) {
    throw new Error(
      `缺少 DNS API 凭据，请填写以下任一方案: ${formatCredentialRequirements(provider)}`,
    );
  }

  return { domains, dnsType, provider, credentials };
};

function crc32(buf: Uint8Array) {
  let c = ~0 >>> 0;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i] ?? 0;
    for (let k = 0; k < 8; k++) {
      const mask = -(c & 1);
      c = (c >>> 1) ^ (0xedb88320 & mask);
    }
  }
  return ~c >>> 0;
}

function dtNow() {
  const d = new Date();
  const dosTime =
    ((d.getHours() & 0x1f) << 11) |
    ((d.getMinutes() & 0x3f) << 5) |
    (Math.floor(d.getSeconds() / 2) & 0x1f);
  const dosDate =
    (((d.getFullYear() - 1980) & 0x7f) << 9) |
    (((d.getMonth() + 1) & 0xf) << 5) |
    (d.getDate() & 0x1f);
  return { dosTime, dosDate };
}

function u16(v: number) {
  const b = new Uint8Array(2);
  const dv = new DataView(b.buffer);
  dv.setUint16(0, v, true);
  return b;
}
function u32(v: number) {
  const b = new Uint8Array(4);
  const dv = new DataView(b.buffer);
  dv.setUint32(0, v, true);
  return b;
}

function createZip(entries: { name: string; data: Uint8Array }[]) {
  const files: Uint8Array[] = [];
  const central: Uint8Array[] = [];
  let offset = 0;
  const { dosTime, dosDate } = dtNow();
  for (const e of entries) {
    const nameBytes = new TextEncoder().encode(e.name);
    const csum = crc32(e.data);
    const lfh = new Uint8Array([
      ...u32(0x04034b50),
      ...u16(20),
      ...u16(0),
      ...u16(0),
      ...u16(dosTime),
      ...u16(dosDate),
      ...u32(csum),
      ...u32(e.data.length),
      ...u32(e.data.length),
      ...u16(nameBytes.length),
      ...u16(0),
      ...nameBytes,
      ...e.data,
    ]);
    files.push(lfh);
    const cdfh = new Uint8Array([
      ...u32(0x02014b50),
      ...u16(20),
      ...u16(20),
      ...u16(0),
      ...u16(0),
      ...u16(dosTime),
      ...u16(dosDate),
      ...u32(csum),
      ...u32(e.data.length),
      ...u32(e.data.length),
      ...u16(nameBytes.length),
      ...u16(0),
      ...u16(0),
      ...u16(0),
      ...u16(0),
      ...u32(0),
      ...u32(offset),
      ...nameBytes,
    ]);
    central.push(cdfh);
    offset += lfh.length;
  }
  const centralDir = central.reduce(
    (a, b) => new Uint8Array([...a, ...b]),
    new Uint8Array(),
  );
  const filesBlob = files.reduce(
    (a, b) => new Uint8Array([...a, ...b]),
    new Uint8Array(),
  );
  const eocd = new Uint8Array([
    ...u32(0x06054b50),
    ...u16(0),
    ...u16(0),
    ...u16(entries.length),
    ...u16(entries.length),
    ...u32(centralDir.length),
    ...u32(filesBlob.length),
    ...u16(0),
  ]);
  return new Uint8Array([...filesBlob, ...centralDir, ...eocd]);
}

type AcmeJobNonNull = NonNullable<
  Awaited<ReturnType<typeof configManager.getAcmeJob>>
>;

type AcmeLogAnalysis = {
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

const pickEvidence = (
  logs: string[],
  match: (line: string) => boolean,
  max: number = 3,
) => {
  const hits: string[] = [];
  for (let i = logs.length - 1; i >= 0; i--) {
    const line = logs[i];
    if (!line) continue;
    if (!match(line)) continue;
    hits.push(line);
    if (hits.length >= max) break;
  }
  return hits.length ? hits.reverse() : undefined;
};

const analyzeAcmeLogs = (
  job: AcmeJobNonNull,
  logs: string[],
): AcmeLogAnalysis | null => {
  if (!logs.length) return null;
  const provider = job.provider || undefined;

  const has = (re: RegExp) => logs.some((line) => re.test(line));

  const isCloudflare =
    provider === "dns_cf" || has(/\bCloudflare\b/i) || has(/\bX-Auth-Key\b/i);
  if (isCloudflare) {
    const invalidKey =
      has(/Invalid format for X-Auth-Key header/i) || has(/"code"\s*:\s*6103/i);
    if (invalidKey) {
      return {
        reason: "dns_credentials_invalid",
        provider: "dns_cf",
        message: "Cloudflare API 密钥不正确（X-Auth-Key 格式无效）",
        evidence: pickEvidence(
          logs,
          (line) => /X-Auth-Key/i.test(line) || /"code"\s*:\s*6103/i.test(line),
        ),
      };
    }

    const invalidEmail = has(/Invalid format for X-Auth-Email header/i);
    if (invalidEmail) {
      return {
        reason: "dns_credentials_invalid_email",
        provider: "dns_cf",
        message: "Cloudflare 邮箱不正确（X-Auth-Email 格式无效）",
        evidence: pickEvidence(logs, (line) => /X-Auth-Email/i.test(line)),
      };
    }

    const invalidHeaders =
      has(/Invalid request headers/i) || has(/"code"\s*:\s*6003/i);
    if (invalidHeaders) {
      return {
        reason: "dns_credentials_invalid",
        provider: "dns_cf",
        message: "Cloudflare API 请求头无效，通常是 API 密钥/邮箱不正确导致",
        evidence: pickEvidence(
          logs,
          (line) =>
            /Invalid request headers/i.test(line) ||
            /"code"\s*:\s*6003/i.test(line),
        ),
      };
    }
  }

  const retryAfterLine = [...logs]
    .reverse()
    .find((line) => /retryafter\s*=\s*\d+/i.test(line));
  if (retryAfterLine && /will not retry|too large/i.test(retryAfterLine)) {
    const m = retryAfterLine.match(/retryafter\s*=\s*(\d+)/i);
    const seconds = m ? Number(m[1]) : NaN;
    const isTooLarge = Number.isFinite(seconds) && seconds > 600;
    if (isTooLarge) {
      return {
        reason: "acme_frequency_limited",
        provider,
        message: `申请频率受限（Retry-After=${seconds} 秒，超过 600 秒将不再重试），请等待后再试`,
        evidence: pickEvidence(
          logs,
          (line) =>
            /retryafter\s*=\s*\d+/i.test(line) ||
            /will not retry|too large/i.test(line),
        ),
      };
    }
  }

  const rateLimited = has(/rate limit|too many requests|429/i);
  if (rateLimited) {
    return {
      reason: "dns_api_rate_limited",
      provider,
      message: "DNS API 触发限流（429/Rate limit），稍后重试",
      evidence: pickEvidence(logs, (line) =>
        /rate limit|too many requests|429/i.test(line),
      ),
    };
  }

  const failure = has(/failed|invalid/i);
  if (failure) {
    return {
      reason: "unknown",
      provider,
      message: "日志中检测到错误，但未能自动归因",
      evidence: pickEvidence(logs, (line) => /failed|invalid/i.test(line)),
    };
  }

  return null;
};

const getProviderLabel = (dnsType: string | null | undefined) => {
  const normalized = String(dnsType || "").trim();
  if (!normalized) return "-";
  return (
    dnsProviders.find((provider) => provider.dnsType === normalized)?.label ||
    normalized
  );
};

const ensureInstalledForRequest = async (acme: {
  checkInstalled: () => Promise<boolean>;
  getState: () => { status: string; message: string };
}) => {
  await acme.checkInstalled();
  const state = acme.getState();
  if (state.status === "installed") return state;
  if (state.status === "installing") {
    throw new Error("acme.sh 安装中，请稍后再试");
  }
  throw new Error("请先安装 acme.sh");
};

const getUsableIssuedCertificateForApplication = async (
  application: AcmeApplication,
) => {
  const issuedCertificate = await configManager.getAcmeIssuedCertificate(
    application.id,
  );
  if (
    !configManager.isAcmeIssuedCertificateCompatible(
      application,
      issuedCertificate,
    )
  ) {
    return null;
  }
  return issuedCertificate;
};

const getStatusCertificate = async () => {
  const applications = await configManager.listAcmeApplications();
  for (const application of applications) {
    const issuedCertificate =
      await getUsableIssuedCertificateForApplication(application);
    if (!issuedCertificate) continue;
    return {
      primaryDomain: issuedCertificate.primaryDomain,
      info: issuedCertificate.certInfo,
    };
  }
  return null;
};

const resolveLegacyApplicationForMutation = async (domains: string[]) => {
  const applications = await configManager.listAcmeApplications();
  const primaryDomain = domains[0] || "";
  const matchedApplication = applications.find(
    (application) => application.primaryDomain === primaryDomain,
  );
  if (matchedApplication) return matchedApplication;
  if (applications.length === 1) return applications[0] || null;
  if (applications.length > 1) {
    throw new Error("当前已存在多个申请项，请使用新接口管理 ACME 申请项");
  }
  return null;
};

const buildPendingApplication = (
  application: AcmeApplication,
  input: {
    name?: string;
    domains: string[];
    dnsType: string;
    credentials: Record<string, string>;
    renewEnabled?: boolean;
  },
): AcmeApplication => ({
  ...application,
  name:
    input.name !== undefined
      ? input.name.trim() || undefined
      : application.name,
  domains: input.domains,
  primaryDomain: input.domains[0] || application.primaryDomain,
  dnsType: input.dnsType,
  credentials: input.credentials,
  renewEnabled: input.renewEnabled ?? application.renewEnabled,
});

const syncGatewayIfAcmeLibraryRemoved = async (input: {
  removedActive: boolean;
  removedCount: number;
}) => {
  if (!input.removedActive && input.removedCount <= 0) return;
  const currentConfig = await configManager.getConfig();
  if (
    input.removedActive ||
    (input.removedCount > 0 &&
      currentConfig.ssl.deployment_mode === "multi_sni")
  ) {
    await syncSSLDeploymentToGateway(currentConfig);
  }
};

const syncGatewayIfAcmeApplicationSaveRemovedLibrary = async (
  saved: Pick<
    AcmeApplicationSaveResult,
    "removedActiveLibraryCertificate" | "removedLibraryCertificates"
  >,
) => {
  await syncGatewayIfAcmeLibraryRemoved({
    removedActive: saved.removedActiveLibraryCertificate,
    removedCount: saved.removedLibraryCertificates.length,
  });
};

const deleteAcmeApplicationCertificate = async (applicationId: string) => {
  const application = await configManager.getAcmeApplication(applicationId);
  if (!application) {
    throw new Error("申请项不存在");
  }

  const issuedCertificate =
    await configManager.getAcmeIssuedCertificate(applicationId);
  const deletedFromLibrary =
    await configManager.deleteSSLCertificatesBySourceRef("acme", applicationId);
  await configManager.deleteAcmeIssuedCertificate(applicationId);

  const { join } = await import("node:path");
  const { rm } = await import("node:fs/promises");
  const domainsToRemove = new Set(
    [application.primaryDomain, issuedCertificate?.primaryDomain].filter(
      (value): value is string => Boolean(value),
    ),
  );

  for (const domain of domainsToRemove) {
    await rm(join(process.cwd(), "data", "ssl", domain), {
      recursive: true,
      force: true,
    });
  }

  await syncGatewayIfAcmeLibraryRemoved({
    removedActive: deletedFromLibrary.removedActive,
    removedCount: deletedFromLibrary.removed.length,
  });

  return {
    application,
    issuedCertificate,
    deletedFromLibrary,
  };
};

const buildApplicationOverview = async () => {
  const [applications, issuedCertificates, sslStatus] = await Promise.all([
    configManager.listAcmeApplications(),
    configManager.listAcmeIssuedCertificates(),
    configManager.getSSLStatus(),
  ]);

  const applicationMap = new Map(applications.map((item) => [item.id, item]));
  const issuedByApplicationId = new Map(
    issuedCertificates
      .filter((item) =>
        configManager.isAcmeIssuedCertificateCompatible(
          applicationMap.get(item.applicationId),
          item,
        ),
      )
      .map((item) => [item.applicationId, item]),
  );
  const latestJobIds = Array.from(
    new Set(
      applications
        .map((item) => item.latestJobId)
        .filter((item): item is string => Boolean(item)),
    ),
  );
  const latestJobs = await Promise.all(
    latestJobIds.map((jobId) => configManager.getAcmeJob(jobId)),
  );
  const latestJobMap = new Map(
    latestJobs
      .filter((job): job is NonNullable<typeof job> => job !== null)
      .map((job) => [job.id, job]),
  );

  return applications.map((application) => {
    const issuedCertificate = issuedByApplicationId.get(application.id) || null;
    const latestJob = application.latestJobId
      ? latestJobMap.get(application.latestJobId) || null
      : null;
    const libraryCertificate = issuedCertificate
      ? sslStatus.certificates.find(
          (certificate) =>
            certificate.source === "acme" &&
            (certificate.source_ref_id === application.id ||
              (!!issuedCertificate.libraryCertificateId &&
                certificate.id === issuedCertificate.libraryCertificateId)),
        ) || null
      : null;

    return {
      id: application.id,
      name: application.name,
      primaryDomain: application.primaryDomain,
      domains: application.domains,
      dnsType: application.dnsType,
      providerLabel: getProviderLabel(application.dnsType),
      renewEnabled: application.renewEnabled,
      createdAt: application.createdAt,
      updatedAt: application.updatedAt,
      latestJob: latestJob
        ? {
            id: latestJob.id,
            status: latestJob.status,
            trigger: latestJob.trigger || "manual_request",
            createdAt:
              latestJob.startedAt ||
              latestJob.createdAt ||
              application.updatedAt,
            message: latestJob.message,
          }
        : application.latestJobId
          ? {
              id: application.latestJobId,
              status: application.latestJobStatus || "idle",
              trigger: application.latestJobTrigger || "manual_request",
              createdAt: application.latestJobAt || application.updatedAt,
              message: application.lastError,
            }
          : null,
      certificate: issuedCertificate
        ? {
            exists: true,
            validFrom: issuedCertificate.certInfo.validFrom,
            validTo: issuedCertificate.certInfo.validTo,
            dnsNames: issuedCertificate.certInfo.dnsNames,
            issuer: issuedCertificate.certInfo.issuer,
          }
        : {
            exists: false,
          },
      library: libraryCertificate
        ? {
            linked: true,
            certificateId: libraryCertificate.id,
            isActive: libraryCertificate.is_active,
          }
        : {
            linked: false,
          },
    };
  });
};

export const acmeRoutes = new Elysia({ prefix: "/api/admin/acme" })
  .use(acmePlugin)
  .get("/status", async ({ acme }) => {
    await acme.checkInstalled();
    const state = acme.getState();
    const clientSettings = await configManager.ensureAcmeClientSettings(
      await acme.getDefaultCertificateAuthority(),
    );
    return {
      success: true,
      data: {
        ...state,
        acmeCert: await getStatusCertificate(),
        certificateAuthority: clientSettings.certificateAuthority,
        certificateAuthorityUpdatedAt: clientSettings.updatedAt,
      },
    };
  })
  .get("/overview", async ({ acme }) => {
    await acme.checkInstalled();
    const [clientSettings, lock, applications, runningJob] = await Promise.all([
      configManager.ensureAcmeClientSettings(
        await acme.getDefaultCertificateAuthority(),
      ),
      configManager.getActiveAcmeRuntimeLock(),
      buildApplicationOverview(),
      configManager.getActiveAcmeJobFromLock(),
    ]);

    return {
      success: true,
      data: {
        acmeState: acme.getState(),
        clientSettings,
        lock,
        applications,
        runningJob: runningJob
          ? {
              id: runningJob.id,
              applicationId: runningJob.applicationId,
              status: runningJob.status,
              progress: runningJob.progress,
            }
          : null,
      },
    };
  })
  .get("/config", async () => {
    const cfg = await configManager.getAcmeSettings();
    return { success: true, data: cfg };
  })
  .get("/applications", async () => {
    const applications = await configManager.listAcmeApplications();
    return { success: true, data: applications };
  })
  .get("/applications/:id", async ({ params, set }) => {
    const application = await configManager.getAcmeApplication(params.id);
    if (!application) {
      set.status = 404;
      return { success: false, message: "not found" };
    }
    return { success: true, data: application };
  })
  .get("/subdomain-recommendation", async () => {
    const config = await configManager.getConfig();
    return {
      success: true,
      data: buildSubdomainCertificateRecommendation(config),
    };
  })
  .get("/dns-providers", () => {
    return { success: true, data: dnsProviders };
  })
  .delete("/", async ({ acme, set }) => {
    try {
      const st = acme.getState();
      if (st.status === "installing") {
        set.status = 409;
        return { success: false, message: "acme.sh 安装中，无法删除" };
      }
      await acme.uninstall();
      await acme.checkInstalled();
      return { success: true, data: acme.getState() };
    } catch (e: any) {
      set.status = 500;
      return { success: false, message: e?.message || String(e) };
    }
  })
  .post("/init", async ({ acme }) => {
    const clientSettings = await configManager.ensureAcmeClientSettings(
      await acme.getDefaultCertificateAuthority(),
    );
    void acme.startInstall(undefined, clientSettings.certificateAuthority);
    return {
      success: true,
      data: {
        executablePath: acme.getState().executablePath,
        certificateAuthority: clientSettings.certificateAuthority,
      },
    };
  })
  .post(
    "/client-settings",
    async ({ acme, body, set }) => {
      await acme.checkInstalled();
      const state = acme.getState();
      if (state.status === "installing") {
        set.status = 409;
        return {
          success: false,
          message: "acme.sh 安装中，暂时无法切换证书颁发机构",
        };
      }

      const previous = await configManager.ensureAcmeClientSettings(
        await acme.getDefaultCertificateAuthority(),
      );
      const next = await configManager.saveAcmeClientSettings({
        certificateAuthority: body.certificateAuthority,
      });

      if (state.status !== "installed") {
        return {
          success: true,
          data: {
            ...next,
            synced: false,
          },
        };
      }

      try {
        const accountEmail = await acme.switchCertificateAuthority(
          body.certificateAuthority,
        );
        await acme.checkInstalled();
        return {
          success: true,
          data: {
            ...next,
            synced: true,
            accountEmail,
            state: acme.getState(),
          },
        };
      } catch (e: any) {
        await configManager.saveAcmeClientSettings({
          certificateAuthority: previous.certificateAuthority,
        });
        set.status = 500;
        return { success: false, message: e?.message || String(e) };
      }
    },
    {
      body: t.Object({
        certificateAuthority: t.Union([
          t.Literal("zerossl"),
          t.Literal("letsencrypt"),
        ]),
      }),
    },
  )
  .post(
    "/config",
    async ({ body, set }) => {
      try {
        const normalized = validateAndNormalizeAcmeRequest(body);
        const targetApplication = await resolveLegacyApplicationForMutation(
          normalized.domains,
        );
        const saved = await configManager.saveAcmeApplicationWithEffects({
          id: targetApplication?.id,
          name: targetApplication?.name,
          domains: normalized.domains,
          dnsType: normalized.dnsType,
          credentials: normalized.credentials,
          renewEnabled: targetApplication?.renewEnabled ?? true,
        });
        const next = {
          domains: saved.application.domains,
          dnsType: saved.application.dnsType,
          credentials: saved.application.credentials,
          updatedAt: saved.application.updatedAt,
        };
        await syncGatewayIfAcmeApplicationSaveRemovedLibrary(saved);
        return { success: true, data: next };
      } catch (e: any) {
        set.status = 400;
        return { success: false, message: e?.message || String(e) };
      }
    },
    {
      body: t.Object({
        domains: t.Array(t.String(), { minItems: 1 }),
        dnsType: t.String(),
        credentials: t.Optional(t.Record(t.String(), t.String())),
      }),
    },
  )
  .post(
    "/applications",
    async ({ acme, body, set }) => {
      try {
        const normalized = validateAndNormalizeAcmeRequest(body);
        const saved = await configManager.saveAcmeApplicationWithEffects({
          name: body.name,
          domains: normalized.domains,
          dnsType: normalized.dnsType,
          credentials: normalized.credentials,
          renewEnabled: body.renewEnabled,
        });
        const application = saved.application;

        await syncGatewayIfAcmeApplicationSaveRemovedLibrary(saved);

        if (!body.submitNow) {
          return { success: true, data: { application } };
        }

        await ensureInstalledForRequest(acme);
        const started = await startAcmeApplicationJob({
          acme,
          application,
          trigger: "manual_request",
        });
        return {
          success: true,
          data: {
            application,
            job: started.job,
            lock: started.lock,
          },
        };
      } catch (e: any) {
        const message = e?.message || String(e);
        set.status = /稍后再试|请先安装|安装中/.test(message) ? 409 : 400;
        return { success: false, message };
      }
    },
    {
      body: t.Object({
        name: t.Optional(t.String()),
        domains: t.Array(t.String(), { minItems: 1 }),
        dnsType: t.String(),
        credentials: t.Optional(t.Record(t.String(), t.String())),
        renewEnabled: t.Optional(t.Boolean()),
        submitNow: t.Optional(t.Boolean()),
      }),
    },
  )
  .patch(
    "/applications/:id",
    async ({ acme, params, body, set }) => {
      try {
        const existing = await configManager.getAcmeApplication(params.id);
        if (!existing) {
          set.status = 404;
          return { success: false, message: "not found" };
        }

        const normalized = validateAndNormalizeAcmeRequest(body);
        let reservation: Awaited<
          ReturnType<typeof reserveAcmeApplicationJob>
        > | null = null;
        let reservationHandedOff = false;

        if (body.submitNow) {
          await ensureInstalledForRequest(acme);
          reservation = await reserveAcmeApplicationJob({
            application: buildPendingApplication(existing, {
              name: body.name,
              domains: normalized.domains,
              dnsType: normalized.dnsType,
              credentials: normalized.credentials,
              renewEnabled: body.renewEnabled,
            }),
            trigger: "manual_request",
          });
        }

        try {
          const saved = await configManager.saveAcmeApplicationWithEffects({
            id: params.id,
            name: body.name,
            domains: normalized.domains,
            dnsType: normalized.dnsType,
            credentials: normalized.credentials,
            renewEnabled: body.renewEnabled,
          });
          const application = saved.application;

          await syncGatewayIfAcmeApplicationSaveRemovedLibrary(saved);

          if (!body.submitNow) {
            return { success: true, data: { application } };
          }

          const started = reservation
            ? await runReservedAcmeApplicationJob({
                acme,
                application,
                trigger: "manual_request",
                job: reservation.job,
                lock: reservation.lock,
              })
            : await startAcmeApplicationJob({
                acme,
                application,
                trigger: "manual_request",
              });
          reservationHandedOff = reservation !== null;

          return {
            success: true,
            data: {
              application,
              job: started.job,
              lock: started.lock,
            },
          };
        } catch (error: any) {
          if (reservation && !reservationHandedOff) {
            await failReservedAcmeApplicationJob({
              applicationId: existing.id,
              job: reservation.job,
              lock: reservation.lock,
              message: error?.message || String(error),
            });
          }
          throw error;
        }
      } catch (e: any) {
        const message = e?.message || String(e);
        if (message === "not found") {
          set.status = 404;
        } else {
          set.status = /稍后再试|请先安装|安装中/.test(message) ? 409 : 400;
        }
        return { success: false, message };
      }
    },
    {
      body: t.Object({
        name: t.Optional(t.String()),
        domains: t.Array(t.String(), { minItems: 1 }),
        dnsType: t.String(),
        credentials: t.Optional(t.Record(t.String(), t.String())),
        renewEnabled: t.Optional(t.Boolean()),
        submitNow: t.Optional(t.Boolean()),
      }),
    },
  )
  .post("/applications/:id/request", async ({ acme, params, set }) => {
    try {
      await ensureInstalledForRequest(acme);
      const application = await configManager.getAcmeApplication(params.id);
      if (!application) {
        set.status = 404;
        return { success: false, message: "not found" };
      }

      const started = await startAcmeApplicationJob({
        acme,
        application,
        trigger: "manual_request",
      });
      return {
        success: true,
        data: {
          job: started.job,
          lock: started.lock,
        },
      };
    } catch (e: any) {
      const message = e?.message || String(e);
      set.status = /稍后再试|请先安装|安装中/.test(message) ? 409 : 400;
      return { success: false, message };
    }
  })
  .delete("/applications/:id/certificate", async ({ params, set }) => {
    try {
      await deleteAcmeApplicationCertificate(params.id);
      return { success: true };
    } catch (e: any) {
      const message = e?.message || String(e);
      set.status = message === "申请项不存在" ? 404 : 400;
      return { success: false, message };
    }
  })
  .post("/applications/:id/library/sync", async ({ params, set }) => {
    try {
      const application = await configManager.getAcmeApplication(params.id);
      if (!application) {
        set.status = 404;
        return { success: false, message: "not found" };
      }

      const issuedCertificate =
        await getUsableIssuedCertificateForApplication(application);
      if (!issuedCertificate) {
        set.status = 400;
        return {
          success: false,
          message: "当前申请项还没有与域名配置匹配的已签发证书",
        };
      }

      const saved =
        await configManager.saveAcmeCertificateToLibraryByApplication(
          params.id,
          {
            label: application.name || application.primaryDomain,
          },
        );
      const currentConfig = await configManager.getConfig();
      const shouldSyncGateway =
        currentConfig.ssl.active_cert_id === saved.id ||
        currentConfig.ssl.deployment_mode === "multi_sni";
      if (shouldSyncGateway) {
        await syncSSLDeploymentToGateway(currentConfig);
      }

      return {
        success: true,
        data: {
          certificateId: saved.id,
          linked: true,
        },
      };
    } catch (e: any) {
      set.status = 400;
      return { success: false, message: e?.message || String(e) };
    }
  })
  .post("/applications/:id/deploy", async ({ params, set }) => {
    try {
      const application = await configManager.getAcmeApplication(params.id);
      if (!application) {
        set.status = 404;
        return { success: false, message: "not found" };
      }

      const issuedCertificate =
        await getUsableIssuedCertificateForApplication(application);
      if (!issuedCertificate) {
        set.status = 400;
        return {
          success: false,
          message: "当前申请项还没有与域名配置匹配的已签发证书",
        };
      }

      await configManager.saveAcmeCertificateToLibraryByApplication(params.id, {
        label: application.name || application.primaryDomain,
        activate: true,
      });
      await syncSSLDeploymentToGateway();
      return { success: true, message: "成功" };
    } catch (e: any) {
      set.status = 400;
      return { success: false, message: e?.message || String(e) };
    }
  })
  .post(
    "/request",
    async ({ acme, body, set }) => {
      try {
        const method = body.method ?? "dns";
        if (method !== "dns") {
          set.status = 400;
          return { success: false, message: "仅支持 DNS-01 验证方式" };
        }

        await ensureInstalledForRequest(acme);
        const normalized = validateAndNormalizeAcmeRequest({
          domains: body.domains,
          dnsType: body.dnsType,
          provider: body.provider,
          credentials: body.credentials,
        });
        const targetApplication = await resolveLegacyApplicationForMutation(
          normalized.domains,
        );
        let reservation: Awaited<
          ReturnType<typeof reserveAcmeApplicationJob>
        > | null = null;
        let reservationHandedOff = false;

        if (targetApplication) {
          reservation = await reserveAcmeApplicationJob({
            application: buildPendingApplication(targetApplication, {
              name: targetApplication.name,
              domains: normalized.domains,
              dnsType: normalized.dnsType,
              credentials: normalized.credentials,
              renewEnabled: targetApplication.renewEnabled,
            }),
            trigger: "manual_request",
          });
        }

        let started:
          | Awaited<ReturnType<typeof startAcmeApplicationJob>>
          | Awaited<ReturnType<typeof runReservedAcmeApplicationJob>>;

        try {
          const saved = await configManager.saveAcmeApplicationWithEffects({
            id: targetApplication?.id,
            name: targetApplication?.name,
            domains: normalized.domains,
            dnsType: normalized.dnsType,
            credentials: normalized.credentials,
            renewEnabled: targetApplication?.renewEnabled ?? true,
          });
          const application = saved.application;
          await syncGatewayIfAcmeApplicationSaveRemovedLibrary(saved);
          started = reservation
            ? await runReservedAcmeApplicationJob({
                acme,
                application,
                trigger: "manual_request",
                job: reservation.job,
                lock: reservation.lock,
              })
            : await startAcmeApplicationJob({
                acme,
                application,
                trigger: "manual_request",
              });
          reservationHandedOff = reservation !== null;
        } catch (error: any) {
          if (reservation && !reservationHandedOff) {
            await failReservedAcmeApplicationJob({
              applicationId: targetApplication.id,
              job: reservation.job,
              lock: reservation.lock,
              message: error?.message || String(error),
            });
          }
          throw error;
        }

        return { success: true, data: { jobId: started.job.id } };
      } catch (e: any) {
        const message = e?.message || String(e);
        set.status = /稍后再试|请先安装|安装中/.test(message) ? 409 : 400;
        return { success: false, message };
      }
    },
    {
      body: t.Object({
        domains: t.Array(t.String(), { minItems: 1 }),
        method: t.Optional(
          t.Union([t.Literal("dns"), t.Literal("http"), t.Literal("https")]),
        ),
        provider: t.Optional(t.String()),
        dnsType: t.Optional(t.String()),
        credentials: t.Optional(t.Record(t.String(), t.String())),
      }),
    },
  )
  .get(
    "/jobs/:id/poll",
    async ({ params, query, set }) => {
      const job = await configManager.getAcmeJob(params.id);
      if (!job) {
        set.status = 404;
        return { success: false, message: "not found" };
      }
      const limit = Math.max(
        1,
        Math.min(DEFAULT_REDIS_LOG_BUFFER_MAX_LEN, Number(query.limit ?? 500)),
      );
      const order = query.order === "asc" ? "asc" : "desc";
      const logs = await configManager.getAcmeLogs(params.id, limit, order);
      const analysis = analyzeAcmeLogs(job, logs);
      return { success: true, data: { job, logs, analysis } };
    },
    {
      query: t.Object({
        limit: t.Optional(t.Numeric()),
        order: t.Optional(t.Union([t.Literal("asc"), t.Literal("desc")])),
      }),
    },
  )
  .get("/jobs/:id", async ({ params, set }) => {
    const job = await configManager.getAcmeJob(params.id);
    if (!job) {
      set.status = 404;
      return { success: false, message: "not found" };
    }
    return { success: true, data: job };
  })
  .get("/jobs/:id/logs", async ({ params }) => {
    const logs = await configManager.getAcmeLogs(params.id, 500, "desc");
    return { success: true, data: logs };
  })
  .get("/certs/:domain", async ({ params, set }) => {
    const application = await configManager.getAcmeApplicationByPrimaryDomain(
      params.domain,
    );
    if (application) {
      const issuedCertificate =
        await getUsableIssuedCertificateForApplication(application);
      if (!issuedCertificate) {
        set.status = 404;
        return { success: false, message: "not found" };
      }
      return {
        success: true,
        data: {
          domain: issuedCertificate.primaryDomain,
          info: issuedCertificate.certInfo,
        },
      };
    }

    const cert = await configManager.getAcmeCert(params.domain);
    if (!cert) {
      set.status = 404;
      return { success: false, message: "not found" };
    }
    const info = await configManager.getAcmeCertInfo(params.domain);
    return { success: true, data: { domain: params.domain, info } };
  })
  .delete("/certs/:domain", async ({ params, set }) => {
    try {
      const application = await configManager.getAcmeApplicationByPrimaryDomain(
        params.domain,
      );
      if (application) {
        await deleteAcmeApplicationCertificate(application.id);
        return { success: true };
      }

      const domain = params.domain;
      await configManager.deleteAcmeCert(domain);
      const deletedFromLibrary =
        await configManager.deleteSSLCertificatesBySource("acme", domain);

      const { join } = await import("node:path");
      const { rm } = await import("node:fs/promises");
      await rm(join(process.cwd(), "data", "ssl", domain), {
        recursive: true,
        force: true,
      });

      await syncGatewayIfAcmeLibraryRemoved({
        removedActive: deletedFromLibrary.removedActive,
        removedCount: deletedFromLibrary.removed.length,
      });

      return { success: true };
    } catch (e: any) {
      set.status = 400;
      return { success: false, message: e?.message || String(e) };
    }
  })
  .get("/certs/:domain/download", async ({ params, set }) => {
    const application = await configManager.getAcmeApplicationByPrimaryDomain(
      params.domain,
    );
    const pair = application
      ? await getUsableIssuedCertificateForApplication(application).then(
          (issuedCertificate) =>
            issuedCertificate
              ? {
                  cert: issuedCertificate.cert,
                  key: issuedCertificate.key,
                }
              : null,
        )
      : await configManager.getAcmeCert(params.domain);
    if (!pair) {
      set.status = 404;
      return { success: false, message: "not found" };
    }
    const entries = [
      {
        name: `${params.domain}.cert.pem`,
        data: new TextEncoder().encode(pair.cert),
      },
      {
        name: `${params.domain}.key.pem`,
        data: new TextEncoder().encode(pair.key),
      },
    ];
    const zipData = createZip(entries);
    return new Response(zipData, {
      headers: {
        "content-type": "application/zip",
        "content-disposition": `attachment; filename="${params.domain}.zip"`,
      },
    });
  })
  .post("/certs/:domain/deploy", async ({ params, set }) => {
    try {
      const application = await configManager.getAcmeApplicationByPrimaryDomain(
        params.domain,
      );
      if (application) {
        const issuedCertificate =
          await getUsableIssuedCertificateForApplication(application);
        if (!issuedCertificate) {
          set.status = 400;
          return {
            success: false,
            message: "当前申请项还没有与域名配置匹配的已签发证书",
          };
        }
        await configManager.saveAcmeCertificateToLibraryByApplication(
          application.id,
          { activate: true },
        );
        await syncSSLDeploymentToGateway();
        return { success: true, message: "成功" };
      }

      const pair = await configManager.getAcmeCert(params.domain);
      if (!pair) {
        return { success: false, message: "证书不存在" };
      }
      const validation = configManager.validateSSLCert(pair.cert, pair.key);
      if (!validation.valid) {
        return {
          success: false,
          message: validation.error || "证书或私钥无效",
        };
      }
      await configManager.saveAcmeCertificateToLibrary(params.domain, {
        activate: true,
      });
      await syncSSLDeploymentToGateway();
      return { success: true, message: "成功" };
    } catch (e: any) {
      set.status = 400;
      return { success: false, message: e?.message || String(e) };
    }
  });
