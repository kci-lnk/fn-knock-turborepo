export type DnsCredentialTarget = "acme" | "ddns";

type CredentialFieldPair = { acme: string; ddns: string; context?: boolean };
export type DnsCredentialBridgeDefinition = {
  id: string;
  labelKey: string;
  acmeDnsType: string;
  ddnsProvider: string;
  fields: CredentialFieldPair[];
};
export type DnsCredentialTransferField = {
  sourceKey: string;
  targetKey: string;
  value: string;
};
export type DnsCredentialTransferSuggestion = {
  bridgeId: string;
  bridgeLabel: string;
  fillableFields: DnsCredentialTransferField[];
  conflictingFields: string[];
  patch: Record<string, string>;
};

const aliyunFields = [
  { acme: "Ali_Key", ddns: "access_key_id" },
  { acme: "Ali_Secret", ddns: "access_key_secret" },
];
const tencentFields = [
  { acme: "Tencent_SecretId", ddns: "secret_id" },
  { acme: "Tencent_SecretKey", ddns: "secret_key" },
];
const bridge = (
  id: string,
  acmeDnsType: string,
  fields: CredentialFieldPair[],
  label = id,
): DnsCredentialBridgeDefinition => ({
  id,
  ddnsProvider: id,
  acmeDnsType,
  fields,
  labelKey: `shared.dnsCredentialBridge.providers.${label}`,
});

export const dnsCredentialBridgeDefinitions = [
  bridge("cloudflare", "dns_cf", [
    { acme: "CF_Token", ddns: "api_token" },
    { acme: "CF_Zone_ID", ddns: "zone_id", context: true },
  ]),
  bridge("alidns", "dns_ali", aliyunFields),
  bridge("esa", "dns_ali", aliyunFields),
  bridge("dnspod", "dns_dp", [
    { acme: "DP_Id", ddns: "token_id" },
    { acme: "DP_Key", ddns: "token_key" },
  ]),
  bridge("dnshe", "dns_dnshe", [
    { acme: "DNSHE_API_KEY", ddns: "api_key" },
    { acme: "DNSHE_API_SECRET", ddns: "api_secret" },
  ]),
  bridge("tencentcloud", "dns_tencent", tencentFields),
  bridge("edgeone", "dns_tencent", tencentFields),
  bridge("edgeone_cname", "dns_tencent", tencentFields, "edgeoneCname"),
  bridge("godaddy", "dns_gd", [
    { acme: "GD_Key", ddns: "api_key" },
    { acme: "GD_Secret", ddns: "api_secret" },
  ]),
  bridge("porkbun", "dns_porkbun", [
    { acme: "PORKBUN_API_KEY", ddns: "api_key" },
    { acme: "PORKBUN_SECRET_API_KEY", ddns: "secret_api_key" },
  ]),
  bridge("dynv6", "dns_dynv6", [{ acme: "DYNV6_TOKEN", ddns: "token" }]),
  bridge("duckdns", "dns_duckdns", [{ acme: "DuckDNS_Token", ddns: "token" }]),
  bridge("baiducloud", "dns_baiducloud", [
    { acme: "BAIDU_ACCESS_KEY_ID", ddns: "access_key_id" },
    { acme: "BAIDU_SECRET_ACCESS_KEY", ddns: "secret_access_key" },
  ]),
];

export const resolveDnsCredentialBridges = (
  target: DnsCredentialTarget,
  providerId: string,
) =>
  dnsCredentialBridgeDefinitions.filter(
    (item) =>
      (target === "acme" ? item.acmeDnsType : item.ddnsProvider) ===
      providerId.trim(),
  );

export const buildDnsCredentialTransferSuggestion = ({
  target,
  providerId,
  sourceProviderId,
  sourceCredentials,
  targetCredentials,
  translateBridgeLabel,
}: {
  target: DnsCredentialTarget;
  providerId: string;
  sourceProviderId: string;
  sourceCredentials: Record<string, string>;
  targetCredentials: Record<string, string>;
  translateBridgeLabel?: (key: string) => string;
}): DnsCredentialTransferSuggestion | null => {
  const match = resolveDnsCredentialBridges(target, providerId).find(
    (item) =>
      (target === "acme" ? item.ddnsProvider : item.acmeDnsType) ===
      sourceProviderId,
  );
  if (!match) return null;
  const mappings = match.fields.map((field) => ({
    sourceKey: target === "acme" ? field.ddns : field.acme,
    targetKey: target === "acme" ? field.acme : field.ddns,
    context: field.context,
  }));
  // A zone identifier alone is not a reusable authentication scheme.
  if (
    !mappings.some(
      (field) => !field.context && sourceCredentials[field.sourceKey]?.trim(),
    )
  ) {
    return null;
  }
  const fillableFields: DnsCredentialTransferField[] = [];
  const conflictingFields: string[] = [];
  for (const field of mappings) {
    const value = sourceCredentials[field.sourceKey]?.trim() || "";
    const existing = targetCredentials[field.targetKey]?.trim() || "";
    // For authentication pairs, an absent source value cannot establish account identity.
    if (existing && existing !== value && (!field.context || value)) {
      conflictingFields.push(field.targetKey);
    }
    if (value && !existing) {
      fillableFields.push({
        sourceKey: field.sourceKey,
        targetKey: field.targetKey,
        value,
      });
    }
  }
  if (target === "acme") {
    const alternateKeys =
      providerId === "dns_cf"
        ? ["CF_Key", "CF_Email"]
        : providerId === "dns_dynv6"
          ? ["KEY"]
          : [];
    conflictingFields.push(
      ...alternateKeys.filter((key) => targetCredentials[key]?.trim()),
    );
  }
  if (!fillableFields.length) return null;
  return {
    bridgeId: match.id,
    bridgeLabel: translateBridgeLabel?.(match.labelKey) ?? match.labelKey,
    fillableFields,
    conflictingFields,
    patch: conflictingFields.length
      ? {}
      : Object.fromEntries(
          fillableFields.map((field) => [field.targetKey, field.value]),
        ),
  };
};
