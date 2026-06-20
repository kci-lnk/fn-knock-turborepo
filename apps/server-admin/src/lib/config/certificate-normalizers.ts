import { createHash } from "node:crypto";
import { redisT } from "./messages";
import { normalizeOptionalString } from "./normalizers";
import type {
  AcmeApplication,
  AcmeApplicationLatestJobStatus,
  AcmeIssuedCertificate,
  AcmeJob,
  AcmeJobTrigger,
  AcmeRuntimeLock,
  SSLCertInfo,
  SSLConfig,
  SSLDeploymentMode,
  SSLManagedCertificate,
  SSLCertificateSource,
} from "./types";

export const normalizeTimestamp = (value: unknown): string => {
  if (typeof value !== "string") return "";
  const trimmed = value.trim();
  if (!trimmed) return "";
  const parsed = Date.parse(trimmed);
  return Number.isFinite(parsed) ? new Date(parsed).toISOString() : "";
};

export const normalizeStringRecord = (
  value: unknown,
): Record<string, string> => {
  if (!value || typeof value !== "object") return {};
  const next: Record<string, string> = {};
  for (const [key, rawValue] of Object.entries(
    value as Record<string, unknown>,
  )) {
    const normalizedKey = String(key ?? "").trim();
    const normalizedValue = String(rawValue ?? "").trim();
    if (!normalizedKey || !normalizedValue) continue;
    next[normalizedKey] = normalizedValue;
  }
  return next;
};

export const normalizeDomainList = (value: unknown): string[] => {
  if (!Array.isArray(value)) return [];
  const domains: string[] = [];
  const seen = new Set<string>();
  for (const raw of value) {
    const domain = String(raw ?? "")
      .trim()
      .toLowerCase();
    if (!domain || seen.has(domain)) continue;
    seen.add(domain);
    domains.push(domain);
  }
  return domains;
};

export const buildNormalizedDomainSignature = (domains: string[]): string =>
  [...normalizeDomainList(domains)]
    .sort((a, b) => a.localeCompare(b))
    .join("\n");

export const hasSameNormalizedDomainSet = (
  left: string[],
  right: string[],
): boolean =>
  buildNormalizedDomainSignature(left) ===
  buildNormalizedDomainSignature(right);

export const normalizeSSLCertInfoValue = (
  value: unknown,
): SSLCertInfo | null => {
  if (!value || typeof value !== "object") return null;
  const raw = value as Partial<SSLCertInfo>;
  const issuer = typeof raw.issuer === "string" ? raw.issuer.trim() : "";
  const subject = typeof raw.subject === "string" ? raw.subject.trim() : "";
  const validFrom =
    typeof raw.validFrom === "string" ? raw.validFrom.trim() : "";
  const validTo = typeof raw.validTo === "string" ? raw.validTo.trim() : "";
  const serialNumber =
    typeof raw.serialNumber === "string" ? raw.serialNumber.trim() : "";
  const dnsNames = normalizeDomainList(raw.dnsNames);

  if (!issuer || !subject || !validFrom || !validTo || !serialNumber) {
    return null;
  }

  return {
    issuer,
    subject,
    validFrom,
    validTo,
    dnsNames,
    serialNumber,
  };
};

export const normalizeSSLCertificateSource = (
  value: unknown,
): SSLCertificateSource => {
  if (value === "acme") return "acme";
  if (value === "ca") return "ca";
  return "manual";
};

export const normalizeSSLDeploymentMode = (
  value: unknown,
): SSLDeploymentMode => (value === "multi_sni" ? "multi_sni" : "single_active");

export const buildSSLCertificateId = (cert: string, key: string): string =>
  `ssl_${createHash("sha256")
    .update(cert)
    .update("\n")
    .update(key)
    .digest("hex")
    .slice(0, 16)}`;

export const normalizeCertificateLabel = ({
  value,
  primaryDomain,
  source,
}: {
  value: unknown;
  primaryDomain?: string;
  source: SSLCertificateSource;
}): string => {
  if (typeof value === "string" && value.trim()) return value.trim();
  if (primaryDomain) return primaryDomain;
  if (source === "acme") return redisT("certificateLabels.acme");
  if (source === "ca") return redisT("certificateLabels.ca");
  return redisT("certificateLabels.manual");
};

export const normalizeManagedSSLCertificate = (
  value?: Partial<SSLManagedCertificate> | null,
): SSLManagedCertificate | null => {
  const raw = value ?? {};
  const cert = typeof raw.cert === "string" ? raw.cert.trim() : "";
  const key = typeof raw.key === "string" ? raw.key.trim() : "";
  if (!cert || !key) return null;

  const source = normalizeSSLCertificateSource(raw.source);
  const primaryDomain =
    typeof raw.primary_domain === "string"
      ? raw.primary_domain.trim().toLowerCase()
      : "";
  const createdAt =
    normalizeTimestamp(raw.created_at) || "1970-01-01T00:00:00.000Z";
  const updatedAt = normalizeTimestamp(raw.updated_at) || createdAt;

  return {
    id:
      typeof raw.id === "string" && raw.id.trim()
        ? raw.id.trim()
        : buildSSLCertificateId(cert, key),
    label: normalizeCertificateLabel({
      value: raw.label,
      primaryDomain: primaryDomain || undefined,
      source,
    }),
    source,
    primary_domain: primaryDomain || undefined,
    source_ref_id: normalizeOptionalString(raw.source_ref_id),
    cert,
    key,
    created_at: createdAt,
    updated_at: updatedAt,
  };
};

export const normalizeAcmeJobTrigger = (
  value: unknown,
): AcmeJobTrigger | undefined => {
  if (value === "manual_request") return "manual_request";
  if (value === "auto_renew") return "auto_renew";
  return undefined;
};

export const normalizeAcmeApplicationLatestJobStatus = (
  value: unknown,
): AcmeApplicationLatestJobStatus | undefined => {
  if (value === "idle") return "idle";
  if (value === "queued") return "queued";
  if (value === "running") return "running";
  if (value === "succeeded") return "succeeded";
  if (value === "failed") return "failed";
  if (value === "stopped") return "stopped";
  return undefined;
};

export const normalizeAcmeJob = (value: unknown): AcmeJob | null => {
  if (!value || typeof value !== "object") return null;
  const raw = value as Partial<AcmeJob>;
  const id = typeof raw.id === "string" ? raw.id.trim() : "";
  const domains = normalizeDomainList(raw.domains);
  const createdAt = normalizeTimestamp(raw.createdAt);
  const status =
    raw.status === "queued" ||
    raw.status === "running" ||
    raw.status === "succeeded" ||
    raw.status === "failed" ||
    raw.status === "stopped"
      ? raw.status
      : undefined;
  if (!id || !domains.length || !createdAt || !status) return null;

  return {
    id,
    applicationId: normalizeOptionalString(raw.applicationId),
    domains,
    method:
      raw.method === "http" || raw.method === "https" ? raw.method : "dns",
    provider:
      typeof raw.provider === "string" && raw.provider.trim()
        ? raw.provider.trim()
        : null,
    trigger: normalizeAcmeJobTrigger(raw.trigger),
    createdAt,
    startedAt: normalizeOptionalString(raw.startedAt),
    finishedAt: normalizeOptionalString(raw.finishedAt),
    status,
    progress:
      typeof raw.progress === "number" && Number.isFinite(raw.progress)
        ? Math.max(0, Math.min(100, Math.round(raw.progress)))
        : 0,
    message: normalizeOptionalString(raw.message),
  };
};

export const normalizeAcmeApplication = (
  value: unknown,
): AcmeApplication | null => {
  if (!value || typeof value !== "object") return null;
  const raw = value as Partial<AcmeApplication>;
  const id = typeof raw.id === "string" ? raw.id.trim() : "";
  const domains = normalizeDomainList(raw.domains);
  const primaryDomain =
    typeof raw.primaryDomain === "string"
      ? raw.primaryDomain.trim().toLowerCase()
      : domains[0] || "";
  const dnsType = typeof raw.dnsType === "string" ? raw.dnsType.trim() : "";
  const createdAt = normalizeTimestamp(raw.createdAt);
  const updatedAt = normalizeTimestamp(raw.updatedAt) || createdAt;

  if (!id || !domains.length || !primaryDomain || !dnsType || !createdAt) {
    return null;
  }

  return {
    id,
    name: normalizeOptionalString(raw.name),
    domains,
    primaryDomain,
    dnsType,
    credentials: normalizeStringRecord(raw.credentials),
    renewEnabled: raw.renewEnabled !== false,
    createdAt,
    updatedAt,
    latestJobId: normalizeOptionalString(raw.latestJobId),
    latestJobStatus: normalizeAcmeApplicationLatestJobStatus(
      raw.latestJobStatus,
    ),
    latestJobTrigger: normalizeAcmeJobTrigger(raw.latestJobTrigger),
    latestJobAt: normalizeOptionalString(raw.latestJobAt),
    lastError: normalizeOptionalString(raw.lastError),
  };
};

export const normalizeAcmeIssuedCertificate = (
  value: unknown,
): AcmeIssuedCertificate | null => {
  if (!value || typeof value !== "object") return null;
  const raw = value as Partial<AcmeIssuedCertificate>;
  const applicationId =
    typeof raw.applicationId === "string" ? raw.applicationId.trim() : "";
  const primaryDomain =
    typeof raw.primaryDomain === "string"
      ? raw.primaryDomain.trim().toLowerCase()
      : "";
  const cert = typeof raw.cert === "string" ? raw.cert.trim() : "";
  const key = typeof raw.key === "string" ? raw.key.trim() : "";
  const createdAt = normalizeTimestamp(raw.createdAt);
  const updatedAt = normalizeTimestamp(raw.updatedAt) || createdAt;
  const certInfo = normalizeSSLCertInfoValue(raw.certInfo);

  if (
    !applicationId ||
    !primaryDomain ||
    !cert ||
    !key ||
    !createdAt ||
    !certInfo
  ) {
    return null;
  }

  return {
    applicationId,
    primaryDomain,
    cert,
    key,
    certInfo,
    createdAt,
    updatedAt,
    libraryCertificateId: normalizeOptionalString(raw.libraryCertificateId),
    libraryLinkedAt: normalizeOptionalString(raw.libraryLinkedAt),
  };
};

export const normalizeAcmeRuntimeLock = (value: unknown): AcmeRuntimeLock => {
  if (!value || typeof value !== "object") return { locked: false };
  const raw = value as Partial<AcmeRuntimeLock>;
  if (raw.locked !== true) return { locked: false };
  return {
    locked: true,
    lockId: normalizeOptionalString(raw.lockId),
    jobId: normalizeOptionalString(raw.jobId),
    applicationId: normalizeOptionalString(raw.applicationId),
    reason: normalizeAcmeJobTrigger(raw.reason),
    startedAt: normalizeOptionalString(raw.startedAt),
    heartbeatAt: normalizeOptionalString(raw.heartbeatAt),
    expiresAt: normalizeOptionalString(raw.expiresAt),
  };
};

export const findMatchingSSLCertificate = (
  certificates: SSLManagedCertificate[],
  cert: string,
  key: string,
): SSLManagedCertificate | null =>
  certificates.find((item) => item.cert === cert && item.key === key) || null;

export const normalizeSSLConfig = (
  value?: Partial<SSLConfig> | null,
): SSLConfig => {
  const raw = value ?? {};
  const certificates = Array.isArray(raw.certificates)
    ? raw.certificates
        .map((item) => normalizeManagedSSLCertificate(item))
        .filter((item): item is SSLManagedCertificate => item !== null)
    : [];

  const normalizedCertificates: SSLManagedCertificate[] = [];
  const seenIds = new Set<string>();
  for (const certificate of certificates) {
    if (seenIds.has(certificate.id)) continue;
    seenIds.add(certificate.id);
    normalizedCertificates.push(certificate);
  }

  const legacyCert = typeof raw.cert === "string" ? raw.cert.trim() : "";
  const legacyKey = typeof raw.key === "string" ? raw.key.trim() : "";
  let legacyMatch: SSLManagedCertificate | null = null;

  if (legacyCert && legacyKey) {
    legacyMatch = findMatchingSSLCertificate(
      normalizedCertificates,
      legacyCert,
      legacyKey,
    );

    if (!legacyMatch) {
      const migrated = normalizeManagedSSLCertificate({
        id: buildSSLCertificateId(legacyCert, legacyKey),
        label: redisT("certificateLabels.current"),
        source: "manual",
        cert: legacyCert,
        key: legacyKey,
      });
      if (migrated) {
        normalizedCertificates.unshift(migrated);
        legacyMatch = migrated;
      }
    }
  }

  const activeFromId =
    typeof raw.active_cert_id === "string" && raw.active_cert_id.trim()
      ? normalizedCertificates.find(
          (item) => item.id === raw.active_cert_id?.trim(),
        ) || null
      : null;
  const activeCertificate = activeFromId || legacyMatch || null;

  return {
    cert: activeCertificate?.cert || "",
    key: activeCertificate?.key || "",
    active_cert_id: activeCertificate?.id || "",
    deployment_mode: normalizeSSLDeploymentMode(raw.deployment_mode),
    certificates: normalizedCertificates,
  };
};

export const mirrorActiveSSLCertificate = (
  ssl: SSLConfig,
  activeCertId?: string | null,
): SSLConfig => {
  const normalized = normalizeSSLConfig(ssl);
  const active =
    activeCertId && activeCertId.trim()
      ? normalized.certificates?.find((item) => item.id === activeCertId) ||
        null
      : null;

  return {
    ...normalized,
    cert: active?.cert || "",
    key: active?.key || "",
    active_cert_id: active?.id || "",
  };
};
