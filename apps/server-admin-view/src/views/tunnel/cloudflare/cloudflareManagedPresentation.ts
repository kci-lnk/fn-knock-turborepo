import type {
  CloudflareReconcileConflict,
  CloudflareReconcileOperation,
} from "@/lib/api/tunnel";
import type { CloudflareTranslate } from "./cloudflareTunnelTypes";

const operationKindKeys: Record<string, string> = {
  tunnel: "tunnel",
  ingress: "ingress",
  dns: "dns",
  optimization: "optimization",
  "custom-hostname": "customHostname",
  permission: "permission",
};
const operationActionKeys: Record<string, string> = {
  create: "create",
  update: "update",
  delete: "delete",
  keep: "keep",
  "keep-deleted": "keepDeleted",
  fallback: "fallback",
  probe: "probe",
  recover: "recover",
};
const operationTargetKeys: Record<string, string> = {
  "optimization:cleanup":
    "admin.cloudflareTunnel.managed.operationTargets.optimizedHostnames",
  "dns:wildcard-dns":
    "admin.cloudflareTunnel.managed.operationTargets.managedWildcardCname",
};
const legacyPlanWarningCodes: Record<string, string> = {
  "Optimization is a Beta feature measured from this server's network vantage point.":
    "betaVantage",
  "Built-in and custom third-party hostnames are used only to discover candidate Cloudflare IPs. Business DNS is never pointed at those hostnames.":
    "candidateDiscoveryOnly",
  "Cloudflare for SaaS includes up to 100 exact Custom Hostnames on non-Enterprise plans; excess domains use the wildcard Tunnel.":
    "customHostnameQuota",
  "The wildcard Tunnel remains configured and is restored automatically if the preferred edge path fails.":
    "wildcardFallback",
};
const planWarningKeys: Record<string, string> = {
  betaVantage: "admin.cloudflareTunnel.managed.planWarnings.betaVantage",
  candidateDiscoveryOnly:
    "admin.cloudflareTunnel.managed.planWarnings.candidateDiscoveryOnly",
  customHostnameQuota:
    "admin.cloudflareTunnel.managed.planWarnings.customHostnameQuota",
  wildcardFallback:
    "admin.cloudflareTunnel.managed.planWarnings.wildcardFallback",
};
const legacyConflictMessageCodes: Record<string, string> = {
  "The managed Tunnel ingress changed after fn-knock last wrote it":
    "managedIngressChanged",
  "The previously managed DNS record has been claimed or changed by another configuration":
    "managedDnsChanged",
  "An unowned Tunnel ingress rule already uses this hostname": "unownedIngress",
  "An unowned DNS record already uses this hostname": "unownedDns",
  "The previously managed fallback origin has been changed by another configuration":
    "fallbackOriginChanged",
  "A previously managed Custom Hostname was changed by another configuration":
    "managedCustomHostnameChanged",
  "The previously managed capability Custom Hostname was changed by another configuration":
    "capabilityHostnameChanged",
  "A previously managed optimization DNS record has been claimed or changed by another configuration":
    "managedOptimizationDnsChanged",
  "A Zone-wide fallback origin already exists and is not owned by fn-knock":
    "unownedFallbackOrigin",
  "An unowned Cloudflare for SaaS Custom Hostname already exists":
    "unownedCustomHostname",
  "An unowned exact DNS record prevents optimization": "exactDnsConflict",
  "An unowned DNS record already uses the optimization hostname":
    "optimizationDnsConflict",
  "Multiple exact DNS records must be resolved before optimization":
    "multipleExactDnsConflict",
  "Multiple DNS records already use the optimization hostname":
    "multipleOptimizationDnsConflict",
};
const conflictMessageKeys: Record<string, string> = {
  managedIngressChanged:
    "admin.cloudflareTunnel.managed.conflictMessages.managedIngressChanged",
  managedDnsChanged:
    "admin.cloudflareTunnel.managed.conflictMessages.managedDnsChanged",
  unownedIngress:
    "admin.cloudflareTunnel.managed.conflictMessages.unownedIngress",
  unownedDns: "admin.cloudflareTunnel.managed.conflictMessages.unownedDns",
  cloudflareSaasUnavailable:
    "admin.cloudflareTunnel.managed.conflictMessages.cloudflareSaasUnavailable",
  permissionError:
    "admin.cloudflareTunnel.managed.conflictMessages.permissionError",
  fallbackOriginChanged:
    "admin.cloudflareTunnel.managed.conflictMessages.fallbackOriginChanged",
  managedCustomHostnameChanged:
    "admin.cloudflareTunnel.managed.conflictMessages.managedCustomHostnameChanged",
  capabilityHostnameChanged:
    "admin.cloudflareTunnel.managed.conflictMessages.capabilityHostnameChanged",
  managedOptimizationDnsChanged:
    "admin.cloudflareTunnel.managed.conflictMessages.managedOptimizationDnsChanged",
  unownedFallbackOrigin:
    "admin.cloudflareTunnel.managed.conflictMessages.unownedFallbackOrigin",
  unownedCustomHostname:
    "admin.cloudflareTunnel.managed.conflictMessages.unownedCustomHostname",
  exactDnsConflict:
    "admin.cloudflareTunnel.managed.conflictMessages.exactDnsConflict",
  optimizationDnsConflict:
    "admin.cloudflareTunnel.managed.conflictMessages.optimizationDnsConflict",
  multipleExactDnsConflict:
    "admin.cloudflareTunnel.managed.conflictMessages.multipleExactDnsConflict",
  multipleOptimizationDnsConflict:
    "admin.cloudflareTunnel.managed.conflictMessages.multipleOptimizationDnsConflict",
};
const capabilityKeys: Record<string, string> = {
  zoneRead: "zoneRead",
  tunnelEdit: "tunnelEdit",
  dnsEdit: "dnsEdit",
  sslCertificatesEdit: "sslCertificatesEdit",
};
const tunnelStatusKeys: Record<string, string> = {
  healthy: "healthy",
  degraded: "degraded",
  down: "down",
  inactive: "inactive",
};

export const formatCloudflareManagedDate = (
  value: string | null | undefined,
  locale: string,
) => {
  if (!value) return "-";
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime())
    ? value
    : parsed.toLocaleString(locale);
};
export const managedOperationKindLabel = (
  value: string,
  t: CloudflareTranslate,
) => {
  const key = operationKindKeys[value];
  return key ? t(`admin.cloudflareTunnel.managed.operationKinds.${key}`) : value;
};
export const managedOperationActionLabel = (
  value: string,
  t: CloudflareTranslate,
) => {
  const key = operationActionKeys[value];
  return key
    ? t(`admin.cloudflareTunnel.managed.operationActions.${key}`)
    : value;
};
export const managedOperationTargetLabel = (
  operation: CloudflareReconcileOperation,
  t: CloudflareTranslate,
) => {
  const key = operationTargetKeys[operation.id];
  return key ? t(key) : operation.target;
};
export const managedConflictTargetLabel = (
  conflict: CloudflareReconcileConflict,
  t: CloudflareTranslate,
) => {
  if (conflict.id === "permission:ssl-certificates") {
    return t("admin.cloudflareTunnel.managed.capabilities.sslCertificatesEdit");
  }
  if (
    conflict.id === "optimization:fallback-origin" ||
    conflict.id === "optimization:cleanup-fallback-origin"
  ) {
    return t("admin.cloudflareTunnel.managed.conflictTargets.fallbackOrigin");
  }
  return conflict.target;
};
export const managedConflictMessageLabel = (
  conflict: CloudflareReconcileConflict,
  t: CloudflareTranslate,
) => {
  const code =
    conflict.messageCode ?? legacyConflictMessageCodes[conflict.message];
  const key = code ? conflictMessageKeys[code] : undefined;
  return key ? t(key, { detail: conflict.detail || "" }) : conflict.message;
};
export const optimizationConflictHostname = (
  conflict: CloudflareReconcileConflict,
) =>
  conflict.id === `optimization:dns:${conflict.target}`
    ? conflict.target
    : null;
export const managedDnsOwnerLabel = (
  owner: NonNullable<
    CloudflareReconcileConflict["details"]
  >["records"][number]["ownerKind"],
  t: CloudflareTranslate,
) => t(`admin.cloudflareTunnel.managed.dnsConflict.ownerKinds.${owner}`);
export const managedDnsProxyLabel = (
  proxied: boolean | null,
  t: CloudflareTranslate,
) =>
  proxied === true
    ? t("admin.cloudflareTunnel.managed.dnsConflict.proxied")
    : proxied === false
      ? t("admin.cloudflareTunnel.managed.dnsConflict.dnsOnly")
      : t("admin.cloudflareTunnel.managed.dnsConflict.unknownProxy");
export const managedCapabilityLabel = (
  value: string,
  t: CloudflareTranslate,
) => {
  const key = capabilityKeys[value];
  return key ? t(`admin.cloudflareTunnel.managed.capabilities.${key}`) : value;
};
export const managedTunnelStatusLabel = (
  status: string | null | undefined,
  t: CloudflareTranslate,
) => {
  if (!status) return "-";
  const key = tunnelStatusKeys[status];
  return key
    ? t(`admin.cloudflareTunnel.managed.tunnelStatuses.${key}`)
    : status;
};
export const managedCapabilityStatusLabel = (
  capability: { required: boolean; readable: boolean | null },
  t: CloudflareTranslate,
) => {
  if (!capability.required) {
    return t("admin.cloudflareTunnel.managed.capabilityNotRequired");
  }
  return capability.readable
    ? t("admin.cloudflareTunnel.managed.capabilityReadable")
    : t("admin.cloudflareTunnel.managed.capabilityMissing");
};
export const managedPlanWarningLabel = (
  warning: string,
  code: string | undefined,
  t: CloudflareTranslate,
) => {
  const resolvedCode = code ?? legacyPlanWarningCodes[warning];
  const key = resolvedCode ? planWarningKeys[resolvedCode] : undefined;
  return key ? t(key) : warning;
};
