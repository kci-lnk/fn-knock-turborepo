import { useI18n } from "vue-i18n";
import { type AcmeApplicationOverviewItem } from "@/lib/api/acme";

type CertificateStatusKey =
  "none" | "invalid" | "expired" | "expiring" | "valid";

type UseAcmeCertificateDisplayOptions = {
  isActionBlocked: () => boolean;
};

export function useAcmeCertificateDisplay({
  isActionBlocked,
}: UseAcmeCertificateDisplayOptions) {
  const { locale, t } = useI18n();

  const primaryActionLabel = (application: AcmeApplicationOverviewItem) => {
    return application.certificate?.exists
      ? t("admin.acmeCert.reapply")
      : t("admin.acmeCert.apply");
  };

  const isSecondaryActionDisabled = (
    application: AcmeApplicationOverviewItem,
  ) => {
    return isActionBlocked() && !application.latestJob?.id;
  };

  const certificateStatusKey = (
    application: AcmeApplicationOverviewItem,
  ): CertificateStatusKey => {
    if (!application.certificate?.exists) return "none";
    const validTo = Date.parse(application.certificate.validTo || "");
    if (!Number.isFinite(validTo)) return "invalid";
    if (validTo <= Date.now()) return "expired";
    if (validTo - Date.now() <= 30 * 24 * 60 * 60 * 1000) return "expiring";
    return "valid";
  };

  const certificateStatusLabel = (application: AcmeApplicationOverviewItem) => {
    const key = certificateStatusKey(application);
    return t(`admin.acmeCert.certificateStatus.${key}`);
  };

  const certificateBadgeVariant = (
    application: AcmeApplicationOverviewItem,
  ) => {
    const key = certificateStatusKey(application);
    if (key === "none") return "outline";
    if (key === "valid") return "secondary";
    return "destructive";
  };

  const formatCertificateRange = (application: AcmeApplicationOverviewItem) => {
    if (!application.certificate?.exists) return t("admin.acmeCert.notIssued");
    const validFrom = application.certificate.validFrom || "";
    const validTo = application.certificate.validTo || "";
    if (!validFrom || !validTo) {
      return t("admin.acmeCert.certificateInfoInvalid");
    }
    return `${formatDate(validFrom)} ~ ${formatDate(validTo)}`;
  };

  const latestJobLabel = (application: AcmeApplicationOverviewItem) => {
    const status = application.latestJob?.status;
    if (!status || status === "idle") return t("admin.acmeCert.jobStatus.idle");
    if (status === "queued") return t("admin.acmeCert.jobStatus.queued");
    if (status === "running") return t("admin.acmeCert.jobStatus.running");
    if (status === "succeeded") return t("admin.acmeCert.jobStatus.succeeded");
    if (status === "failed") return t("admin.acmeCert.jobStatus.failed");
    if (status === "stopped") return t("admin.acmeCert.jobStatus.stopped");
    return status;
  };

  const jobBadgeVariant = (status?: string | null) => {
    if (!status || status === "idle") return "outline";
    if (status === "queued") return "outline";
    if (status === "running") return "default";
    if (status === "succeeded") return "secondary";
    if (status === "failed") return "outline";
    if (status === "stopped") return "outline";
    return "outline";
  };

  const libraryStatusLabel = (application: AcmeApplicationOverviewItem) => {
    if (application.library?.isActive)
      return t("admin.acmeCert.library.active");
    if (application.library?.linked) return t("admin.acmeCert.library.linked");
    return t("admin.acmeCert.library.unlinked");
  };

  const libraryBadgeVariant = (application: AcmeApplicationOverviewItem) => {
    if (application.library?.isActive) return "default";
    if (application.library?.linked) return "secondary";
    return "outline";
  };

  const deleteApplicationDescription = (
    application: AcmeApplicationOverviewItem,
  ) => {
    const target = application.name || application.primaryDomain;
    if (application.certificate?.exists || application.library?.linked) {
      return t("admin.acmeCert.deleteApplicationWithCertificateDescription", {
        target,
      });
    }
    return t("admin.acmeCert.deleteApplicationDescription", { target });
  };

  const formatDate = (value: string) => {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value;
    return date.toLocaleString(locale.value);
  };

  return {
    certificateBadgeVariant,
    certificateStatusLabel,
    deleteApplicationDescription,
    formatCertificateRange,
    jobBadgeVariant,
    latestJobLabel,
    libraryBadgeVariant,
    libraryStatusLabel,
    primaryActionLabel,
    isSecondaryActionDisabled,
  };
}
