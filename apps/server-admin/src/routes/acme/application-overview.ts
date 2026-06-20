import { getProviderLabel } from "../../lib/acme-dns-providers";
import { configManager } from "../../lib/redis";

type TranslationParams = Record<
  string,
  string | number | boolean | null | undefined
>;

type Translator = (key: string, params?: TranslationParams) => string;

export const buildApplicationOverview = async (t: Translator) => {
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
