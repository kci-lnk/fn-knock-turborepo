import type {
  DDNSTargetMeta,
  DDNSTargetRecord,
  DDNSTargetSummary,
} from "./types";
import {
  DDNS_UPDATE_SCOPE_FIELD,
  ddnsTranslate,
  normalizeUpdateScope,
} from "./providers/helpers";

type ProviderLabelResolver = (name: string | null | undefined) => string;

export const compareDDNSTargets = (
  left: DDNSTargetMeta,
  right: DDNSTargetMeta,
): number => {
  if (left.isPrimary !== right.isPrimary) {
    return left.isPrimary ? -1 : 1;
  }
  if (left.sortOrder !== right.sortOrder) {
    return left.sortOrder - right.sortOrder;
  }
  if (left.createdAt !== right.createdAt) {
    return left.createdAt.localeCompare(right.createdAt);
  }
  return left.id.localeCompare(right.id);
};

export const buildDDNSDomainSummary = (
  providerName: string | null | undefined,
  config: Record<string, string>,
): string => {
  const provider = providerName?.trim() || "";
  const candidates = [
    config.domain,
    config.hostname,
    config.domains,
    config.zone,
    config.root_domain,
    config.site_name,
    config.site_id,
  ];
  const summary = candidates.find((value) => value?.trim())?.trim() || "";

  if (summary) {
    return summary;
  }

  return provider ? "" : ddnsTranslate("noProviderSelected");
};

export const buildDDNSTargetDisplayName = (
  meta: DDNSTargetMeta,
  providerLabel: string,
  domainSummary: string,
): string => {
  const explicitName = meta.name.trim();
  if (explicitName) {
    return explicitName;
  }
  if (meta.isPrimary) {
    return ddnsTranslate("primaryDomainName");
  }
  return domainSummary || providerLabel;
};

export const buildDDNSTargetDuplicateKey = (
  providerName: string | null | undefined,
  config: Record<string, string>,
): string => {
  const normalizedProviderName = providerName?.trim() || "";
  const normalizedDomainSummary = buildDDNSDomainSummary(
    normalizedProviderName,
    config,
  )
    .trim()
    .toLowerCase();

  if (!normalizedProviderName || !normalizedDomainSummary) {
    return "";
  }

  return `${normalizedProviderName}::${normalizedDomainSummary}`;
};

export const toDDNSTargetSummary = (
  target: DDNSTargetRecord,
  getProviderLabel: ProviderLabelResolver,
): DDNSTargetSummary => {
  const providerLabel = getProviderLabel(target.provider);
  const domainSummary = buildDDNSDomainSummary(target.provider, target.config);

  return {
    id: target.id,
    name: buildDDNSTargetDisplayName(target, providerLabel, domainSummary),
    isPrimary: target.isPrimary,
    enabled: target.isPrimary ? true : target.enabled,
    provider: target.provider,
    updateScope: normalizeUpdateScope(target.config[DDNS_UPDATE_SCOPE_FIELD]),
    providerLabel,
    domainSummary,
    createdAt: target.createdAt,
    updatedAt: target.updatedAt,
    sortOrder: target.sortOrder,
    lastIP: target.lastIP,
    lastCheck: target.lastCheck,
  };
};

export const buildDDNSTargetLogLabel = (
  target: Pick<
    DDNSTargetRecord | DDNSTargetSummary,
    "id" | "isPrimary" | "name" | "provider"
  >,
  getProviderLabel: ProviderLabelResolver,
  config?: Record<string, string>,
): string => {
  const providerLabel = getProviderLabel(target.provider);
  const domainSummary =
    "domainSummary" in target
      ? target.domainSummary
      : buildDDNSDomainSummary(target.provider, config || {});
  const label = domainSummary || target.name || providerLabel;
  const scope = target.isPrimary
    ? ddnsTranslate("primaryDomainScope")
    : ddnsTranslate("additionalDomainScope");
  return `[${scope}][${providerLabel}]${label ? `[${label}]` : ""}`;
};
