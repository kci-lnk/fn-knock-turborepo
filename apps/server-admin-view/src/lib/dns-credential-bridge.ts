export type DnsCredentialTarget = "acme" | "ddns";

type DnsCredentialMapping = {
  from: string;
  to: string;
};

type DnsCredentialBridgeDefinition = {
  id: string;
  labelKey: string;
  acmeDnsType: string;
  ddnsProvider: string;
  acmeToDdns: DnsCredentialMapping[];
  ddnsToAcme: DnsCredentialMapping[];
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
  patch: Record<string, string>;
};

const dnsCredentialBridgeDefinitions: DnsCredentialBridgeDefinition[] = [
  {
    id: "cloudflare",
    labelKey: "shared.dnsCredentialBridge.providers.cloudflare",
    acmeDnsType: "dns_cf",
    ddnsProvider: "cloudflare",
    acmeToDdns: [
      { from: "CF_Token", to: "api_token" },
      { from: "CF_Zone_ID", to: "zone_id" },
    ],
    ddnsToAcme: [
      { from: "api_token", to: "CF_Token" },
      { from: "zone_id", to: "CF_Zone_ID" },
    ],
  },
  {
    id: "alidns",
    labelKey: "shared.dnsCredentialBridge.providers.alidns",
    acmeDnsType: "dns_ali",
    ddnsProvider: "alidns",
    acmeToDdns: [
      { from: "Ali_Key", to: "access_key_id" },
      { from: "Ali_Secret", to: "access_key_secret" },
    ],
    ddnsToAcme: [
      { from: "access_key_id", to: "Ali_Key" },
      { from: "access_key_secret", to: "Ali_Secret" },
    ],
  },
  {
    id: "dnspod",
    labelKey: "shared.dnsCredentialBridge.providers.dnspod",
    acmeDnsType: "dns_dp",
    ddnsProvider: "dnspod",
    acmeToDdns: [
      { from: "DP_Id", to: "token_id" },
      { from: "DP_Key", to: "token_key" },
    ],
    ddnsToAcme: [
      { from: "token_id", to: "DP_Id" },
      { from: "token_key", to: "DP_Key" },
    ],
  },
  {
    id: "tencentcloud",
    labelKey: "shared.dnsCredentialBridge.providers.tencentcloud",
    acmeDnsType: "dns_tencent",
    ddnsProvider: "tencentcloud",
    acmeToDdns: [
      { from: "Tencent_SecretId", to: "secret_id" },
      { from: "Tencent_SecretKey", to: "secret_key" },
    ],
    ddnsToAcme: [
      { from: "secret_id", to: "Tencent_SecretId" },
      { from: "secret_key", to: "Tencent_SecretKey" },
    ],
  },
  {
    id: "edgeone",
    labelKey: "shared.dnsCredentialBridge.providers.edgeone",
    acmeDnsType: "dns_tencent",
    ddnsProvider: "edgeone",
    acmeToDdns: [
      { from: "Tencent_SecretId", to: "secret_id" },
      { from: "Tencent_SecretKey", to: "secret_key" },
    ],
    ddnsToAcme: [
      { from: "secret_id", to: "Tencent_SecretId" },
      { from: "secret_key", to: "Tencent_SecretKey" },
    ],
  },
  {
    id: "edgeone_cname",
    labelKey: "shared.dnsCredentialBridge.providers.edgeoneCname",
    acmeDnsType: "dns_tencent",
    ddnsProvider: "edgeone_cname",
    acmeToDdns: [
      { from: "Tencent_SecretId", to: "secret_id" },
      { from: "Tencent_SecretKey", to: "secret_key" },
    ],
    ddnsToAcme: [
      { from: "secret_id", to: "Tencent_SecretId" },
      { from: "secret_key", to: "Tencent_SecretKey" },
    ],
  },
  {
    id: "godaddy",
    labelKey: "shared.dnsCredentialBridge.providers.godaddy",
    acmeDnsType: "dns_gd",
    ddnsProvider: "godaddy",
    acmeToDdns: [
      { from: "GD_Key", to: "api_key" },
      { from: "GD_Secret", to: "api_secret" },
    ],
    ddnsToAcme: [
      { from: "api_key", to: "GD_Key" },
      { from: "api_secret", to: "GD_Secret" },
    ],
  },
  {
    id: "porkbun",
    labelKey: "shared.dnsCredentialBridge.providers.porkbun",
    acmeDnsType: "dns_porkbun",
    ddnsProvider: "porkbun",
    acmeToDdns: [
      { from: "PORKBUN_API_KEY", to: "api_key" },
      { from: "PORKBUN_SECRET_API_KEY", to: "secret_api_key" },
    ],
    ddnsToAcme: [
      { from: "api_key", to: "PORKBUN_API_KEY" },
      { from: "secret_api_key", to: "PORKBUN_SECRET_API_KEY" },
    ],
  },
  {
    id: "dynv6",
    labelKey: "shared.dnsCredentialBridge.providers.dynv6",
    acmeDnsType: "dns_dynv6",
    ddnsProvider: "dynv6",
    acmeToDdns: [{ from: "DYNV6_TOKEN", to: "token" }],
    ddnsToAcme: [{ from: "token", to: "DYNV6_TOKEN" }],
  },
  {
    id: "duckdns",
    labelKey: "shared.dnsCredentialBridge.providers.duckdns",
    acmeDnsType: "dns_duckdns",
    ddnsProvider: "duckdns",
    acmeToDdns: [{ from: "DuckDNS_Token", to: "token" }],
    ddnsToAcme: [{ from: "token", to: "DuckDNS_Token" }],
  },
];

const normalizeValue = (value: string | null | undefined) =>
  value?.trim() || "";

export const resolveDnsCredentialBridge = (
  target: DnsCredentialTarget,
  providerId: string,
) => {
  const normalizedProviderId = providerId.trim();
  if (!normalizedProviderId) return null;

  return (
    dnsCredentialBridgeDefinitions.find((bridge) =>
      target === "acme"
        ? bridge.acmeDnsType === normalizedProviderId
        : bridge.ddnsProvider === normalizedProviderId,
    ) || null
  );
};

export const buildDnsCredentialTransferSuggestion = ({
  target,
  providerId,
  sourceCredentials,
  targetCredentials,
  translateBridgeLabel,
}: {
  target: DnsCredentialTarget;
  providerId: string;
  sourceCredentials: Record<string, string>;
  targetCredentials: Record<string, string>;
  translateBridgeLabel?: (key: string) => string;
}): DnsCredentialTransferSuggestion | null => {
  const bridge = resolveDnsCredentialBridge(target, providerId);
  if (!bridge) return null;

  const mappings = target === "acme" ? bridge.ddnsToAcme : bridge.acmeToDdns;
  const fillableFields = mappings.flatMap((mapping) => {
    const sourceValue = normalizeValue(sourceCredentials[mapping.from]);
    if (!sourceValue) return [];

    const existingTargetValue = normalizeValue(targetCredentials[mapping.to]);
    if (existingTargetValue) return [];

    return [
      {
        sourceKey: mapping.from,
        targetKey: mapping.to,
        value: sourceValue,
      },
    ];
  });

  if (fillableFields.length === 0) return null;

  return {
    bridgeId: bridge.id,
    bridgeLabel: translateBridgeLabel?.(bridge.labelKey) ?? bridge.labelKey,
    fillableFields,
    patch: Object.fromEntries(
      fillableFields.map((field) => [field.targetKey, field.value]),
    ),
  };
};
