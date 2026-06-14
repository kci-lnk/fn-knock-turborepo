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
  stopActiveAcmeApplicationJob,
} from "../lib/acme-job-runner";
import {
  createAcmeDnsProviders,
  dnsProviders,
  filterAcmeCredentialsForProvider,
  formatCredentialRequirements,
  getProviderLabel,
  getSatisfiedCredentialScheme,
  normalizeAcmeDnsType,
} from "../lib/acme-dns-providers";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import { createRequestTranslator, tDefault } from "../lib/i18n";

type RequestTranslator = ReturnType<typeof createRequestTranslator>["t"];

const getAcmeRouteTranslator = async (request: Request) =>
  createRequestTranslator(request, await configManager.getLocaleConfig()).t;

const acmeRouteT = (
  t: RequestTranslator,
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => t(`server.acmeRoutes.${key}`, params);

const acmeDefaultT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.acmeRoutes.${key}`, params);

const isAcmeConflictMessage = (
  message: string,
  t?: RequestTranslator,
): boolean => {
  const messages = [
    acmeDefaultT("installingRetryLater"),
    acmeDefaultT("installFirst"),
    tDefault("server.acmeService.installFirst"),
    tDefault("server.acmeService.installingCannotDelete"),
    tDefault("server.acmeJobRunner.activeTaskRunning"),
  ];
  if (t) {
    messages.push(
      acmeRouteT(t, "installingRetryLater"),
      acmeRouteT(t, "installFirst"),
      t("server.acmeService.installFirst"),
      t("server.acmeService.installingCannotDelete"),
      t("server.acmeJobRunner.activeTaskRunning"),
    );
  }
  return messages.includes(message);
};

const isAcmeApplicationNotFoundMessage = (
  message: string,
  t?: RequestTranslator,
): boolean => {
  const messages = [
    acmeDefaultT("applicationNotFound"),
    tDefault("server.redis.acme.applicationNotFound"),
  ];
  if (t) {
    messages.push(
      acmeRouteT(t, "applicationNotFound"),
      t("server.redis.acme.applicationNotFound"),
    );
  }
  return messages.includes(message);
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

const validateAndNormalizeAcmeRequest = (
  input: {
    domains: string[];
    dnsType?: string;
    provider?: string;
    credentials?: Record<string, string>;
  },
  t: RequestTranslator,
) => {
  const domains = normalizeDomains(input.domains);
  if (domains.length === 0) throw new Error(acmeRouteT(t, "domainsInvalid"));

  const dnsType = normalizeAcmeDnsType(input.dnsType ?? input.provider);
  if (!dnsType) throw new Error(acmeRouteT(t, "dnsTypeRequired"));
  const provider = dnsProviders.find((p) => p.dnsType === dnsType) || null;
  if (!provider) throw new Error(acmeRouteT(t, "unsupportedDnsProvider"));

  const credentials = filterAcmeCredentialsForProvider(
    provider,
    input.credentials,
  );
  const matchedScheme = getSatisfiedCredentialScheme(provider, credentials);
  if (!matchedScheme) {
    throw new Error(
      acmeRouteT(t, "missingDnsCredentials", {
        requirements: formatCredentialRequirements(provider, t),
      }),
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
  t: RequestTranslator,
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
        message: acmeRouteT(t, "cloudflareInvalidKey"),
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
        message: acmeRouteT(t, "cloudflareInvalidEmail"),
        evidence: pickEvidence(logs, (line) => /X-Auth-Email/i.test(line)),
      };
    }

    const invalidHeaders =
      has(/Invalid request headers/i) || has(/"code"\s*:\s*6003/i);
    if (invalidHeaders) {
      return {
        reason: "dns_credentials_invalid",
        provider: "dns_cf",
        message: acmeRouteT(t, "cloudflareInvalidHeaders"),
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
        message: acmeRouteT(t, "acmeFrequencyLimited", { seconds }),
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
      message: acmeRouteT(t, "dnsApiRateLimited"),
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
      message: acmeRouteT(t, "logUnknownFailure"),
      evidence: pickEvidence(logs, (line) => /failed|invalid/i.test(line)),
    };
  }

  return null;
};

const ensureInstalledForRequest = async (
  acme: {
    checkInstalled: () => Promise<boolean>;
    getState: () => { status: string; message: string };
  },
  t: RequestTranslator,
) => {
  await acme.checkInstalled();
  const state = acme.getState();
  if (state.status === "installed") return state;
  if (state.status === "installing") {
    throw new Error(acmeRouteT(t, "installingRetryLater"));
  }
  throw new Error(acmeRouteT(t, "installFirst"));
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

const resolveLegacyApplicationForMutation = async (
  domains: string[],
  t: RequestTranslator,
) => {
  const applications = await configManager.listAcmeApplications();
  const primaryDomain = domains[0] || "";
  const matchedApplication = applications.find(
    (application) => application.primaryDomain === primaryDomain,
  );
  if (matchedApplication) return matchedApplication;
  if (applications.length === 1) return applications[0] || null;
  if (applications.length > 1) {
    throw new Error(acmeRouteT(t, "multipleApplicationsUseNewApi"));
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

const deleteAcmeApplicationCertificate = async (
  applicationId: string,
  t: RequestTranslator,
) => {
  const application = await configManager.getAcmeApplication(applicationId);
  if (!application) {
    throw new Error(acmeRouteT(t, "applicationNotFound"));
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

const deleteAcmeApplication = async (
  applicationId: string,
  t: RequestTranslator,
) => {
  const deleted = await configManager.deleteAcmeApplication(applicationId);
  if (!deleted) {
    throw new Error(acmeRouteT(t, "applicationNotFound"));
  }

  await syncGatewayIfAcmeLibraryRemoved({
    removedActive: deleted.removedActiveLibraryCertificate,
    removedCount: deleted.removedLibraryCertificates.length,
  });

  return deleted;
};

const buildApplicationOverview = async (t: RequestTranslator) => {
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
      providerLabel: getProviderLabel(application.dnsType, t),
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

export const acmeRoutes = new Elysia({
  prefix: "/api/admin/acme",
  tags: ["ACME"],
})
  .use(acmePlugin)
  .get(
    "/status",
    async ({ acme, request }) => {
      const t = await getAcmeRouteTranslator(request);
      await acme.checkInstalled();
      const clientSettings = await configManager.ensureAcmeClientSettings(
        await acme.getDefaultCertificateAuthority(),
      );
      return {
        success: true,
        data: {
          ...acme.getLocalizedState(t),
          acmeCert: await getStatusCertificate(),
          certificateAuthority: clientSettings.certificateAuthority,
          certificateAuthorityUpdatedAt: clientSettings.updatedAt,
        },
      };
    },
    routeDoc("获取 ACME 客户端状态"),
  )
  .get(
    "/overview",
    async ({ acme, request }) => {
      const t = await getAcmeRouteTranslator(request);
      await acme.checkInstalled();
      const [clientSettings, lock, applications, runningJob] =
        await Promise.all([
          configManager.ensureAcmeClientSettings(
            await acme.getDefaultCertificateAuthority(),
          ),
          configManager.getActiveAcmeRuntimeLock(),
          buildApplicationOverview(t),
          configManager.getActiveAcmeJobFromLock(),
        ]);

      return {
        success: true,
        data: {
          acmeState: acme.getLocalizedState(t),
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
    },
    routeDoc("获取 ACME 总览"),
  )
  .get(
    "/config",
    async () => {
      const cfg = await configManager.getAcmeSettings();
      return { success: true, data: cfg };
    },
    routeDoc("获取 ACME 配置"),
  )
  .get(
    "/applications",
    async () => {
      const applications = await configManager.listAcmeApplications();
      return { success: true, data: applications };
    },
    routeDoc("获取 ACME 申请项列表"),
  )
  .get(
    "/applications/:id",
    async ({ params, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      const application = await configManager.getAcmeApplication(params.id);
      if (!application) {
        set.status = 404;
        return { success: false, message: acmeRouteT(t, "notFound") };
      }
      return { success: true, data: application };
    },
    routeDoc("获取单个 ACME 申请项"),
  )
  .get(
    "/subdomain-recommendation",
    async ({ request }) => {
      const t = await getAcmeRouteTranslator(request);
      const config = await configManager.getConfig();
      return {
        success: true,
        data: buildSubdomainCertificateRecommendation(config, t),
      };
    },
    routeDoc("获取子域证书推荐"),
  )
  .get(
    "/dns-providers",
    async ({ request }) => {
      const t = await getAcmeRouteTranslator(request);
      return { success: true, data: createAcmeDnsProviders(t) };
    },
    routeDoc("获取 DNS 提供商目录"),
  )
  .delete(
    "/",
    async ({ acme, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      try {
        const st = acme.getState();
        if (st.status === "installing") {
          set.status = 409;
          return {
            success: false,
            message: acmeRouteT(t, "installingCannotDelete"),
          };
        }
        await acme.uninstall();
        await acme.checkInstalled();
        return { success: true, data: acme.getLocalizedState(t) };
      } catch (e: any) {
        set.status = 500;
        return { success: false, message: e?.message || String(e) };
      }
    },
    routeDoc("卸载 ACME 客户端"),
  )
  .post(
    "/init",
    async ({ acme }) => {
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
    },
    routeDoc("初始化并安装 ACME 客户端"),
  )
  .post(
    "/client-settings",
    async ({ acme, body, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      await acme.checkInstalled();
      const state = acme.getState();
      if (state.status === "installing") {
        set.status = 409;
        return {
          success: false,
          message: acmeRouteT(t, "installingCannotSwitchCa"),
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
            state: acme.getLocalizedState(t),
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
    withRouteDoc("切换 ACME 证书颁发机构", {
      body: t.Object({
        certificateAuthority: t.Union([
          t.Literal("zerossl"),
          t.Literal("letsencrypt"),
        ]),
      }),
    }),
  )
  .post(
    "/config",
    async ({ body, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      try {
        const normalized = validateAndNormalizeAcmeRequest(body, t);
        const targetApplication = await resolveLegacyApplicationForMutation(
          normalized.domains,
          t,
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
    withRouteDoc("保存默认 ACME 申请配置", {
      body: t.Object({
        domains: t.Array(t.String(), { minItems: 1 }),
        dnsType: t.String(),
        credentials: t.Optional(t.Record(t.String(), t.String())),
      }),
    }),
  )
  .post(
    "/applications",
    async ({ acme, body, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      try {
        const localeConfig = await configManager.getLocaleConfig();
        const normalized = validateAndNormalizeAcmeRequest(body, t);
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

        await ensureInstalledForRequest(acme, t);
        const started = await startAcmeApplicationJob({
          acme,
          application,
          trigger: "manual_request",
          locale: localeConfig.default_locale,
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
        set.status = isAcmeConflictMessage(message, t) ? 409 : 400;
        return { success: false, message };
      }
    },
    withRouteDoc("创建 ACME 申请项", {
      body: t.Object({
        name: t.Optional(t.String()),
        domains: t.Array(t.String(), { minItems: 1 }),
        dnsType: t.String(),
        credentials: t.Optional(t.Record(t.String(), t.String())),
        renewEnabled: t.Optional(t.Boolean()),
        submitNow: t.Optional(t.Boolean()),
      }),
    }),
  )
  .patch(
    "/applications/:id",
    async ({ acme, params, body, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      try {
        const localeConfig = await configManager.getLocaleConfig();
        const existing = await configManager.getAcmeApplication(params.id);
        if (!existing) {
          set.status = 404;
          return { success: false, message: acmeRouteT(t, "notFound") };
        }

        const normalized = validateAndNormalizeAcmeRequest(body, t);
        let reservation: Awaited<
          ReturnType<typeof reserveAcmeApplicationJob>
        > | null = null;
        let reservationHandedOff = false;

        if (body.submitNow) {
          await ensureInstalledForRequest(acme, t);
          reservation = await reserveAcmeApplicationJob({
            application: buildPendingApplication(existing, {
              name: body.name,
              domains: normalized.domains,
              dnsType: normalized.dnsType,
              credentials: normalized.credentials,
              renewEnabled: body.renewEnabled,
            }),
            trigger: "manual_request",
            locale: localeConfig.default_locale,
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
                locale: localeConfig.default_locale,
              })
            : await startAcmeApplicationJob({
                acme,
                application,
                trigger: "manual_request",
                locale: localeConfig.default_locale,
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
              locale: localeConfig.default_locale,
            });
          }
          throw error;
        }
      } catch (e: any) {
        const message = e?.message || String(e);
        if (message === acmeRouteT(t, "notFound")) {
          set.status = 404;
        } else {
          set.status = isAcmeConflictMessage(message, t) ? 409 : 400;
        }
        return { success: false, message };
      }
    },
    withRouteDoc("更新 ACME 申请项", {
      body: t.Object({
        name: t.Optional(t.String()),
        domains: t.Array(t.String(), { minItems: 1 }),
        dnsType: t.String(),
        credentials: t.Optional(t.Record(t.String(), t.String())),
        renewEnabled: t.Optional(t.Boolean()),
        submitNow: t.Optional(t.Boolean()),
      }),
    }),
  )
  .post(
    "/applications/:id/request",
    async ({ acme, params, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      try {
        const localeConfig = await configManager.getLocaleConfig();
        await ensureInstalledForRequest(acme, t);
        const application = await configManager.getAcmeApplication(params.id);
        if (!application) {
          set.status = 404;
          return { success: false, message: acmeRouteT(t, "notFound") };
        }

        const started = await startAcmeApplicationJob({
          acme,
          application,
          trigger: "manual_request",
          locale: localeConfig.default_locale,
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
        set.status = isAcmeConflictMessage(message, t) ? 409 : 400;
        return { success: false, message };
      }
    },
    routeDoc("立即为申请项发起证书申请"),
  )
  .delete(
    "/applications/:id",
    async ({ params, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      try {
        const lock = await configManager.getActiveAcmeRuntimeLock();
        if (lock.locked) {
          set.status = 409;
          return {
            success: false,
            message: t("server.acmeJobRunner.activeTaskRunning"),
          };
        }

        const deleted = await deleteAcmeApplication(params.id, t);
        return {
          success: true,
          data: {
            id: deleted.application.id,
          },
        };
      } catch (e: any) {
        const message = e?.message || String(e);
        set.status = isAcmeApplicationNotFoundMessage(message, t) ? 404 : 400;
        return { success: false, message };
      }
    },
    routeDoc("删除 ACME 申请项"),
  )
  .delete(
    "/applications/:id/certificate",
    async ({ params, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      try {
        await deleteAcmeApplicationCertificate(params.id, t);
        return { success: true };
      } catch (e: any) {
        const message = e?.message || String(e);
        set.status = isAcmeApplicationNotFoundMessage(message, t) ? 404 : 400;
        return { success: false, message };
      }
    },
    routeDoc("删除申请项已签发证书"),
  )
  .post(
    "/applications/:id/library/sync",
    async ({ params, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      try {
        const application = await configManager.getAcmeApplication(params.id);
        if (!application) {
          set.status = 404;
          return { success: false, message: acmeRouteT(t, "notFound") };
        }

        const issuedCertificate =
          await getUsableIssuedCertificateForApplication(application);
        if (!issuedCertificate) {
          set.status = 400;
          return {
            success: false,
            message: acmeRouteT(t, "noMatchingIssuedCertificate"),
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
    },
    routeDoc("将申请项证书同步到证书库"),
  )
  .post(
    "/applications/:id/deploy",
    async ({ params, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      try {
        const application = await configManager.getAcmeApplication(params.id);
        if (!application) {
          set.status = 404;
          return { success: false, message: acmeRouteT(t, "notFound") };
        }

        const issuedCertificate =
          await getUsableIssuedCertificateForApplication(application);
        if (!issuedCertificate) {
          set.status = 400;
          return {
            success: false,
            message: acmeRouteT(t, "noMatchingIssuedCertificate"),
          };
        }

        await configManager.saveAcmeCertificateToLibraryByApplication(
          params.id,
          {
            label: application.name || application.primaryDomain,
            activate: true,
          },
        );
        await syncSSLDeploymentToGateway();
        return { success: true, message: acmeRouteT(t, "success") };
      } catch (e: any) {
        set.status = 400;
        return { success: false, message: e?.message || String(e) };
      }
    },
    routeDoc("部署申请项证书到网关"),
  )
  .post(
    "/request",
    async ({ acme, body, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      try {
        const method = body.method ?? "dns";
        if (method !== "dns") {
          set.status = 400;
          return { success: false, message: acmeRouteT(t, "dns01Only") };
        }

        await ensureInstalledForRequest(acme, t);
        const normalized = validateAndNormalizeAcmeRequest(
          {
            domains: body.domains,
            dnsType: body.dnsType,
            provider: body.provider,
            credentials: body.credentials,
          },
          t,
        );
        const targetApplication = await resolveLegacyApplicationForMutation(
          normalized.domains,
          t,
        );
        const localeConfig = await configManager.getLocaleConfig();
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
            locale: localeConfig.default_locale,
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
                locale: localeConfig.default_locale,
              })
            : await startAcmeApplicationJob({
                acme,
                application,
                trigger: "manual_request",
                locale: localeConfig.default_locale,
              });
          reservationHandedOff = reservation !== null;
        } catch (error: any) {
          if (reservation && !reservationHandedOff) {
            await failReservedAcmeApplicationJob({
              applicationId: targetApplication?.id ?? "",
              job: reservation.job,
              lock: reservation.lock,
              message: error?.message || String(error),
              locale: localeConfig.default_locale,
            });
          }
          throw error;
        }

        return { success: true, data: { jobId: started.job.id } };
      } catch (e: any) {
        const message = e?.message || String(e);
        set.status = isAcmeConflictMessage(message, t) ? 409 : 400;
        return { success: false, message };
      }
    },
    withRouteDoc("立即申请证书", {
      body: t.Object({
        domains: t.Array(t.String(), { minItems: 1 }),
        method: t.Optional(
          t.Union([t.Literal("dns"), t.Literal("http"), t.Literal("https")]),
        ),
        provider: t.Optional(t.String()),
        dnsType: t.Optional(t.String()),
        credentials: t.Optional(t.Record(t.String(), t.String())),
      }),
    }),
  )
  .post(
    "/jobs/active/stop",
    async ({ acme, set }) => {
      try {
        const localeConfig = await configManager.getLocaleConfig();
        const stopped = await stopActiveAcmeApplicationJob({
          acme,
          locale: localeConfig.default_locale,
        });
        return { success: true, data: stopped };
      } catch (e: any) {
        set.status = 500;
        return { success: false, message: e?.message || String(e) };
      }
    },
    routeDoc("停止当前 ACME 任务并终止 acme.sh 进程"),
  )
  .get(
    "/jobs/:id/poll",
    async ({ params, query, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      const job = await configManager.getAcmeJob(params.id);
      if (!job) {
        set.status = 404;
        return { success: false, message: acmeRouteT(t, "notFound") };
      }
      const limit = Math.max(
        1,
        Math.min(DEFAULT_REDIS_LOG_BUFFER_MAX_LEN, Number(query.limit ?? 500)),
      );
      const order = query.order === "asc" ? "asc" : "desc";
      const logs = await configManager.getAcmeLogs(params.id, limit, order);
      const analysis = analyzeAcmeLogs(job, logs, t);
      return { success: true, data: { job, logs, analysis } };
    },
    withRouteDoc("轮询 ACME 任务状态与日志", {
      query: t.Object({
        limit: t.Optional(t.Numeric()),
        order: t.Optional(t.Union([t.Literal("asc"), t.Literal("desc")])),
      }),
    }),
  )
  .get(
    "/jobs/:id",
    async ({ params, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      const job = await configManager.getAcmeJob(params.id);
      if (!job) {
        set.status = 404;
        return { success: false, message: acmeRouteT(t, "notFound") };
      }
      return { success: true, data: job };
    },
    routeDoc("获取 ACME 任务详情"),
  )
  .get(
    "/jobs/:id/logs",
    async ({ params }) => {
      const logs = await configManager.getAcmeLogs(params.id, 500, "desc");
      return { success: true, data: logs };
    },
    routeDoc("获取 ACME 任务日志"),
  )
  .get(
    "/certs/:domain",
    async ({ params, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      const application = await configManager.getAcmeApplicationByPrimaryDomain(
        params.domain,
      );
      if (application) {
        const issuedCertificate =
          await getUsableIssuedCertificateForApplication(application);
        if (!issuedCertificate) {
          set.status = 404;
          return { success: false, message: acmeRouteT(t, "notFound") };
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
        return { success: false, message: acmeRouteT(t, "notFound") };
      }
      const info = await configManager.getAcmeCertInfo(params.domain);
      return { success: true, data: { domain: params.domain, info } };
    },
    routeDoc("获取域名证书信息"),
  )
  .delete(
    "/certs/:domain",
    async ({ params, request, set }) => {
      const t = await getAcmeRouteTranslator(request);
      try {
        const application =
          await configManager.getAcmeApplicationByPrimaryDomain(params.domain);
        if (application) {
          await deleteAcmeApplicationCertificate(application.id, t);
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
    },
    routeDoc("删除域名证书"),
  )
  .get(
    "/certs/:domain/download",
    async ({ params, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
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
        return { success: false, message: acmeRouteT(t, "notFound") };
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
    },
    routeDoc("下载域名证书压缩包"),
  )
  .post(
    "/certs/:domain/deploy",
    async ({ params, set, request }) => {
      const t = await getAcmeRouteTranslator(request);
      try {
        const application =
          await configManager.getAcmeApplicationByPrimaryDomain(params.domain);
        if (application) {
          const issuedCertificate =
            await getUsableIssuedCertificateForApplication(application);
          if (!issuedCertificate) {
            set.status = 400;
            return {
              success: false,
              message: acmeRouteT(t, "noMatchingIssuedCertificate"),
            };
          }
          await configManager.saveAcmeCertificateToLibraryByApplication(
            application.id,
            { activate: true },
          );
          await syncSSLDeploymentToGateway();
          return { success: true, message: acmeRouteT(t, "success") };
        }

        const pair = await configManager.getAcmeCert(params.domain);
        if (!pair) {
          return { success: false, message: acmeRouteT(t, "certNotFound") };
        }
        const validation = configManager.validateSSLCert(pair.cert, pair.key);
        if (!validation.valid) {
          return {
            success: false,
            message: validation.error || acmeRouteT(t, "certOrKeyInvalid"),
          };
        }
        await configManager.saveAcmeCertificateToLibrary(params.domain, {
          activate: true,
        });
        await syncSSLDeploymentToGateway();
        return { success: true, message: acmeRouteT(t, "success") };
      } catch (e: any) {
        set.status = 400;
        return { success: false, message: e?.message || String(e) };
      }
    },
    routeDoc("部署域名证书到网关"),
  );
