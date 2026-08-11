import type {
  CloudflareOptimizationDomain,
  CloudflareOptimizationVantage,
} from "@/lib/api";
import type { CloudflareTunnelController } from "./useCloudflareTunnelController";

type Translate = CloudflareTunnelController["t"];

const phaseKeys: Record<string, string> = {
  queued: "queued",
  latency: "latency",
  download: "download",
  completed: "completed",
  failed: "failed",
  cancelled: "cancelled",
};

const domainStatusKeys: Record<string, string> = {
  active: "active",
  pending: "pending",
  ready: "ready",
  optimized: "optimized",
  fallback: "fallback",
  conflict: "conflict",
  quota: "quota",
  queued: "queued",
  "probe-failed": "probeFailed",
  external: "external",
};

const switchReasonKeys: Record<string, string> = {
  "manual-speed-test": "manualSpeedTest",
  "manual-fallback": "manualFallback",
  "health-failover": "healthFailover",
  "health-fallback": "healthFallback",
};

export const capabilityStatusKeys: Record<string, string> = {
  pending: "pending",
  "awaiting-candidate": "awaiting-candidate",
  compatible: "compatible",
  unsupported: "unsupported",
};

const legacyDomainMessageCodes: Record<string, string> = {
  "Custom Hostname is not owned by fn-knock":
    "customHostnameOwnershipConflict",
  "Custom Hostname quota is exhausted": "customHostnameQuotaExhausted",
  "Queued to respect Cloudflare certificate issuance rate limits":
    "certificateRateLimited",
};

const domainMessageKeys: Record<string, string> = {
  customHostnameOwnershipConflict:
    "admin.cloudflareTunnel.optimization.domainMessages.customHostnameOwnershipConflict",
  customHostnameQuotaExhausted:
    "admin.cloudflareTunnel.optimization.domainMessages.customHostnameQuotaExhausted",
  certificateRateLimited:
    "admin.cloudflareTunnel.optimization.domainMessages.certificateRateLimited",
  customHostnameQuotaUnavailable:
    "admin.cloudflareTunnel.optimization.domainMessages.customHostnameQuotaUnavailable",
  exactDnsOwnershipConflict:
    "admin.cloudflareTunnel.optimization.domainMessages.exactDnsOwnershipConflict",
  validationDnsOwnershipConflict:
    "admin.cloudflareTunnel.optimization.domainMessages.validationDnsOwnershipConflict",
  preferredEdgeProbeFailed:
    "admin.cloudflareTunnel.optimization.domainMessages.preferredEdgeProbeFailed",
};

export const cloudflareSaasRequiredErrorCode = "cloudflare-saas-required";
export const cloudflareSaasValidationPendingErrorCode =
  "cloudflare-saas-validation-pending";
export const cloudflareResourceConflictErrorCode =
  "cloudflare-resource-conflict";
export const optimizationNotReadyErrorCode =
  "cloudflare-optimization-not-ready";
export const legacyOptimizationNotReadyErrorMarkers = [
  "no active business or capability hostname",
];

const legacyCloudflareSaasErrorMarkers = [
  "not entitled",
  "not enabled for this zone",
  "not available on your plan",
  "plan does not support",
  "requires an enterprise plan",
  "upgrade your plan",
  "no quota has been allocated",
  "(1404)",
];

export const formatOptimizationNumber = (value: number, digits = 1) =>
  Number.isFinite(value) ? value.toFixed(digits) : "-";

export const formatOptimizationDate = (
  value: string | null | undefined,
  locale: string,
) => {
  if (!value) return "-";
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime())
    ? value
    : parsed.toLocaleString(locale);
};

export const requiresCloudflareSaasSetup = (
  errorCode?: string | null,
  message?: string | null,
) => {
  const normalized = message?.toLowerCase();
  if (
    normalized !== undefined &&
    legacyOptimizationNotReadyErrorMarkers.some((marker) =>
      normalized.includes(marker),
    )
  ) {
    return false;
  }
  if (errorCode === cloudflareSaasRequiredErrorCode) return true;
  return (
    normalized !== undefined &&
    legacyCloudflareSaasErrorMarkers.some((marker) =>
      normalized.includes(marker),
    )
  );
};

export const optimizationScanPhaseLabel = (
  phase: string,
  t: Translate,
) => {
  const key = phaseKeys[phase];
  return key ? t(`admin.cloudflareTunnel.optimization.phases.${key}`) : phase;
};

export const optimizationDomainStatusLabel = (
  status: string,
  t: Translate,
) => {
  const key = domainStatusKeys[status];
  return key
    ? t(`admin.cloudflareTunnel.optimization.domainStatuses.${key}`)
    : status;
};

export const optimizationDomainMessageLabel = (
  domain: CloudflareOptimizationDomain,
  t: Translate,
) => {
  if (!domain.message && !domain.messageCode) return "";
  let code =
    domain.messageCode ||
    (domain.message ? legacyDomainMessageCodes[domain.message] : undefined);
  let detail = domain.messageDetail || "";
  const quotaUnavailablePrefix = "Custom Hostname quota is unavailable: ";
  if (!code && domain.message?.startsWith(quotaUnavailablePrefix)) {
    code = "customHostnameQuotaUnavailable";
    detail = domain.message.slice(quotaUnavailablePrefix.length);
  }
  const key = code ? domainMessageKeys[code] : undefined;
  return key ? t(key, { detail }) : domain.message || detail;
};

export const optimizationSwitchReasonLabel = (
  reason: string,
  t: Translate,
) => {
  const key = switchReasonKeys[reason];
  return key
    ? t(`admin.cloudflareTunnel.optimization.switchReasons.${key}`)
    : reason;
};

export const optimizationBuiltinLabel = (
  id: string,
  hostname: string,
  t: Translate,
) => {
  const key = `admin.cloudflareTunnel.optimization.sources.builtins.${id}`;
  const translated = t(key);
  return translated === key ? hostname : translated;
};

export const optimizationCandidateSourceLabel = (
  candidate: { sourceHostnames: string[]; sourceTypes: string[] },
  t: Translate,
) => {
  if (candidate.sourceHostnames.length) {
    return candidate.sourceHostnames.join(", ");
  }
  return candidate.sourceTypes.includes("official-range")
    ? t("admin.cloudflareTunnel.optimization.sources.officialRangesShort")
    : "-";
};

export const optimizationSourceSettingsErrorLabel = (
  message: string,
  t: Translate,
) => {
  const prefix = "Invalid optimization source settings: ";
  return message.startsWith(prefix)
    ? t("admin.cloudflareTunnel.optimization.sources.settingsInvalid", {
        detail: message.slice(prefix.length),
      })
    : message;
};

export const optimizationSourceWarningLabel = (
  warning: string,
  t: Translate,
) => {
  const unverified = warning.match(
    /^(.+) \(([^()]*)\) did not resolve to a verified Cloudflare IPv4 address$/u,
  );
  if (unverified) {
    return t("admin.cloudflareTunnel.optimization.sources.unverifiedAddress", {
      hostname: unverified[1],
      source: unverified[2],
    });
  }
  const separator = warning.indexOf(": ");
  if (separator > 0) {
    return t("admin.cloudflareTunnel.optimization.sources.resolveFailed", {
      hostname: warning.slice(0, separator),
      detail: warning.slice(separator + 2),
    });
  }
  return warning;
};

export const optimizationVantageLabel = (
  vantage: CloudflareOptimizationVantage,
  t: Translate,
) =>
  vantage.id === "local-server"
    ? t("admin.cloudflareTunnel.optimization.vantages.localServer")
    : vantage.label;
