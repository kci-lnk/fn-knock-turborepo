import { computed, type Ref } from "vue";
import type {
  SSLCertificateSource,
  SSLCertificateSummary,
  SSLDeploymentMode,
  SSLStatus,
  SubdomainCertificateCoverage,
} from "@/types";

type TranslationParams = Record<string, string | number>;

export type DeploymentPreviewItem = {
  id: string;
  label: string;
  isDefault: boolean;
};

export type GatewayCertificateItem = NonNullable<
  SSLStatus["gateway_status"]
>["certificates"][number];

export const useCertConfigViewModel = ({
  formData,
  locale,
  sslStatus,
  translate,
}: {
  formData: Ref<{ cert: string; key: string }>;
  locale: Ref<string>;
  sslStatus: Ref<SSLStatus | null>;
  translate: (key: string, params?: TranslationParams) => string;
}) => {
  const certificates = computed(() => sslStatus.value?.certificates || []);
  const activeCertificate = computed(
    () =>
      certificates.value.find((certificate) => certificate.is_active) || null,
  );
  const deployedGatewayCertificates = computed(
    () => sslStatus.value?.gateway_status?.certificates || [],
  );
  const subdomainCoverage = computed(
    () =>
      sslStatus.value?.subdomain_coverage ??
      activeCertificate.value?.coverage ??
      null,
  );
  const libraryCoverage = computed(
    () => sslStatus.value?.library_coverage ?? null,
  );
  const recommendedCertificateId = computed(
    () => libraryCoverage.value?.suggested_certificate_id || "",
  );
  const isExpired = computed(() => {
    const validTo = activeCertificate.value?.certInfo?.validTo;
    if (!validTo) return false;
    return new Date(validTo) < new Date();
  });
  const isExpiringSoon = computed(() => {
    const validTo = activeCertificate.value?.certInfo?.validTo;
    if (!validTo) return false;
    const expiresAt = new Date(validTo);
    const now = new Date();
    const thirtyDays = 30 * 24 * 60 * 60 * 1000;
    return expiresAt > now && expiresAt.getTime() - now.getTime() < thirtyDays;
  });

  const formatDN = (dn: string): string => dn.replace(/\n/g, ", ");

  const formatDate = (dateStr: string): string => {
    if (!dateStr) return "";
    const date = new Date(dateStr);
    if (Number.isNaN(date.getTime())) return dateStr;
    return date.toLocaleDateString(locale.value, {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  const certificateDisplayLabel = (
    certificate: SSLCertificateSummary,
  ): string => {
    return (
      certificate.label ||
      certificate.primary_domain ||
      certificate.certInfo?.dnsNames?.[0] ||
      certificate.id
    );
  };

  const certificateDomainSummary = (
    certificate: SSLCertificateSummary | null,
  ): string => {
    const domains = certificate?.certInfo?.dnsNames || [];
    if (!domains.length) return "";
    const preview = domains.slice(0, 3).join(", ");
    if (domains.length <= 3) return preview;
    return translate("admin.certConfig.domainSummaryMore", {
      preview,
      count: domains.length,
    });
  };

  const gatewayCertificateLabel = (certificate: GatewayCertificateItem) => {
    return (
      certificate.label ||
      certificate.domains?.[0] ||
      certificate.id ||
      translate("admin.certConfig.unnamedCertificate")
    );
  };

  const gatewayCertificateKey = (certificate: GatewayCertificateItem) => {
    return (
      certificate.id ||
      `${gatewayCertificateLabel(certificate)}-${certificate.domains?.join(",") || "no-domains"}`
    );
  };

  const sourceLabel = (source: SSLCertificateSource): string => {
    if (source === "acme") return "ACME";
    if (source === "ca") return translate("admin.certConfig.localCa");
    if (source === "external")
      return translate("admin.certConfig.externalSource");
    return translate("admin.certConfig.manualUploadSource");
  };

  const buildDeploymentPreview = (mode: SSLDeploymentMode) => {
    const items =
      mode === "single_active"
        ? activeCertificate.value
          ? [activeCertificate.value]
          : []
        : [...certificates.value].sort((a, b) => {
            if (a.is_active === b.is_active) return 0;
            return a.is_active ? -1 : 1;
          });

    const defaultCertificate =
      items.find((certificate) => certificate.is_active) || items[0] || null;

    return {
      count: items.length,
      defaultLabel: defaultCertificate
        ? certificateDisplayLabel(defaultCertificate)
        : translate("admin.certConfig.notSet"),
      domainSummary: certificateDomainSummary(defaultCertificate),
      previewItems: items
        .slice(0, 3)
        .map<DeploymentPreviewItem>((certificate) => ({
          id: certificate.id,
          label: certificateDisplayLabel(certificate),
          isDefault: defaultCertificate?.id === certificate.id,
        })),
      remainingCount: Math.max(items.length - 3, 0),
    };
  };

  const primaryCertificateBadgeLabel = computed(() => {
    if (!activeCertificate.value) return translate("common.inactive");
    if (sslStatus.value?.deploymentMode === "multi_sni") {
      return translate("admin.certConfig.defaultFallback");
    }
    return translate("admin.certConfig.enabled");
  });
  const deploymentModeLabel = computed(() => {
    if (sslStatus.value?.deploymentMode === "multi_sni") {
      return translate("admin.certConfig.deploymentModeLabel", {
        mode: translate("admin.certConfig.multiSniTitle"),
      });
    }
    return translate("admin.certConfig.deploymentModeLabel", {
      mode: translate("admin.certConfig.singleActiveTitle"),
    });
  });
  const deploymentModeShortLabel = computed(() => {
    if (sslStatus.value?.deploymentMode === "multi_sni") {
      return translate("admin.certConfig.multiSniTitle");
    }
    return translate("admin.certConfig.singleActiveTitle");
  });
  const configuredDeploymentModeLabel = computed(() => {
    if (sslStatus.value?.configuredDeploymentMode === "multi_sni") {
      return translate("admin.certConfig.multiSniTitle");
    }
    return translate("admin.certConfig.singleActiveTitle");
  });
  const deploymentModeDescription = computed(() => {
    if (sslStatus.value?.deploymentMode === "multi_sni") {
      return translate("admin.certConfig.multiSniModeDescription");
    }
    return translate("admin.certConfig.singleActiveModeDescription");
  });
  const deploymentModeMismatch = computed(
    () =>
      Boolean(sslStatus.value?.configuredDeploymentMode) &&
      sslStatus.value?.configuredDeploymentMode !==
        sslStatus.value?.deploymentMode,
  );
  const gatewaySyncError = computed(
    () => sslStatus.value?.gateway_status?.sync_error || "",
  );
  const showMultiSniSuggestion = computed(
    () =>
      sslStatus.value?.deploymentMode !== "multi_sni" &&
      (libraryCoverage.value?.combined_covering_certificate_ids.length || 0) >
        1,
  );
  const activateButtonLabel = computed(() => {
    if (sslStatus.value?.deploymentMode === "multi_sni") {
      return translate("admin.certConfig.setDefaultCertificate");
    }
    return translate("admin.certConfig.setCurrentCertificate");
  });
  const gatewayDeploymentSummary = computed(() => {
    if (!deployedGatewayCertificates.value.length) {
      return translate("admin.certConfig.noGatewayCertificates");
    }

    const defaultCertificate = deployedGatewayCertificates.value.find(
      (certificate) => certificate.is_default,
    );
    const defaultLabel = defaultCertificate
      ? gatewayCertificateLabel(defaultCertificate)
      : translate("admin.certConfig.notMarked");

    if (sslStatus.value?.deploymentMode === "multi_sni") {
      return translate("admin.certConfig.gatewaySummaryMulti", {
        count: deployedGatewayCertificates.value.length,
        label: defaultLabel,
      });
    }

    return translate("admin.certConfig.gatewaySummarySingle", {
      count: deployedGatewayCertificates.value.length,
      label: defaultLabel,
    });
  });
  const statusOverviewText = computed(() => {
    if (!activeCertificate.value?.certInfo) {
      return translate("admin.certConfig.statusNoActive");
    }

    const parts = [
      translate("admin.certConfig.statusCurrentCertificate", {
        label: certificateDisplayLabel(activeCertificate.value),
      }),
      translate("admin.certConfig.statusSource", {
        source: sourceLabel(activeCertificate.value.source),
      }),
    ];

    if (isExpired.value) {
      parts.push(
        translate("admin.certConfig.statusExpiredAt", {
          date: formatDate(activeCertificate.value.certInfo.validTo),
        }),
      );
    } else if (isExpiringSoon.value) {
      parts.push(
        translate("admin.certConfig.statusExpiringSoonAt", {
          date: formatDate(activeCertificate.value.certInfo.validTo),
        }),
      );
    } else {
      parts.push(
        translate("admin.certConfig.statusValidTo", {
          date: formatDate(activeCertificate.value.certInfo.validTo),
        }),
      );
    }

    return parts.join(" · ");
  });
  const deploymentSummary = computed(() =>
    translate("admin.certConfig.deploymentSummary", {
      mode: deploymentModeShortLabel.value,
      count: deployedGatewayCertificates.value.length,
    }),
  );
  const deploymentSectionConfigured = computed(() =>
    Boolean(
      certificates.value.length || deployedGatewayCertificates.value.length,
    ),
  );
  const currentCertificateSummary = computed(() => {
    if (!activeCertificate.value) {
      return subdomainCoverage.value
        ? translate("admin.certConfig.currentSummaryNoActiveWithCoverage", {
            summary: subdomainCoverage.value.summary,
          })
        : translate("admin.certConfig.noActiveTitle");
    }

    const parts = [certificateDisplayLabel(activeCertificate.value)];
    const domainSummary = certificateDomainSummary(activeCertificate.value);
    if (domainSummary) parts.push(domainSummary);
    if (isExpired.value) {
      parts.push(translate("admin.certConfig.expired"));
    } else if (isExpiringSoon.value) {
      parts.push(translate("admin.certConfig.expiresIn30Days"));
    } else {
      parts.push(translate("common.active"));
    }
    return parts.join(" · ");
  });
  const manualUploadConfigured = computed(() =>
    Boolean(
      certificates.value.length || formData.value.cert || formData.value.key,
    ),
  );
  const manualUploadSummary = computed(() => {
    if (formData.value.cert || formData.value.key) {
      return translate("admin.certConfig.manualUploadFilled");
    }
    if (certificates.value.length) {
      return translate("admin.certConfig.manualUploadHasLibrary", {
        count: certificates.value.length,
      });
    }
    return translate("admin.certConfig.manualUploadEmpty");
  });
  const certificateLibrarySummary = computed(() => {
    const activeLabel = activeCertificate.value
      ? translate("admin.certConfig.libraryCurrentActive", {
          label: certificateDisplayLabel(activeCertificate.value),
        })
      : translate("admin.certConfig.libraryNoActive");
    return translate("admin.certConfig.librarySummary", {
      count: certificates.value.length,
      active: activeLabel,
    });
  });
  const singleActivePreview = computed(() =>
    buildDeploymentPreview("single_active"),
  );
  const multiSniPreview = computed(() => buildDeploymentPreview("multi_sni"));

  const deploymentCardClass = (mode: SSLDeploymentMode) => {
    if (sslStatus.value?.deploymentMode === mode) {
      return "dynamic-white-glass-surface dynamic-white-status-success border-green-500 bg-green-50/60 dark:border-green-500/70 dark:bg-green-950/25";
    }
    return "bg-muted/20";
  };

  const certificateRoleLabel = (certificate: SSLCertificateSummary) => {
    if (!certificate.is_active) return "";
    if (sslStatus.value?.deploymentMode === "multi_sni") {
      return translate("admin.certConfig.defaultFallback");
    }
    return translate("admin.certConfig.currentActive");
  };

  const coverageBadgeVariant = (coverage: SubdomainCertificateCoverage) => {
    if (coverage.status === "missing") return "destructive";
    if (coverage.status === "partial") return "outline";
    return "default";
  };

  const coverageBadgeClass = (coverage: SubdomainCertificateCoverage) => {
    if (coverage.status === "ready") {
      return "dynamic-white-glass-chip dynamic-white-glass-chip-success bg-green-600 hover:bg-green-600";
    }
    if (coverage.status === "partial") {
      return "border-amber-500 text-amber-700 dark:border-amber-400/80 dark:text-amber-300";
    }
    return "";
  };

  const coverageBadgeLabel = (coverage: SubdomainCertificateCoverage) => {
    if (coverage.status === "ready") {
      return translate("admin.certConfig.coverageReady");
    }
    if (coverage.status === "partial") {
      return translate("admin.certConfig.coveragePartial");
    }
    return translate("admin.certConfig.coverageMissing");
  };

  const uncoveredHostsPreview = (hosts: string[]) => {
    if (hosts.length === 0) return "";
    const preview = hosts.slice(0, 4).join(", ");
    if (hosts.length <= 4) return preview;
    return translate("admin.certConfig.uncoveredHostsMore", {
      preview,
      count: hosts.length,
    });
  };

  return {
    activateButtonLabel,
    activeCertificate,
    certificateDisplayLabel,
    certificateLibrarySummary,
    certificateRoleLabel,
    certificates,
    configuredDeploymentModeLabel,
    coverageBadgeClass,
    coverageBadgeLabel,
    coverageBadgeVariant,
    currentCertificateSummary,
    deployedGatewayCertificates,
    deploymentCardClass,
    deploymentModeDescription,
    deploymentModeLabel,
    deploymentModeMismatch,
    deploymentModeShortLabel,
    deploymentSectionConfigured,
    deploymentSummary,
    formatDate,
    formatDN,
    gatewayCertificateKey,
    gatewayCertificateLabel,
    gatewayDeploymentSummary,
    gatewaySyncError,
    isExpired,
    isExpiringSoon,
    libraryCoverage,
    manualUploadConfigured,
    manualUploadSummary,
    multiSniPreview,
    primaryCertificateBadgeLabel,
    recommendedCertificateId,
    showMultiSniSuggestion,
    singleActivePreview,
    sourceLabel,
    statusOverviewText,
    subdomainCoverage,
    uncoveredHostsPreview,
  };
};
