export type DnsCredentialTarget = "acme" | "ddns";

type DnsCredentialMapping = {
  from: string;
  to: string;
};

type DnsCredentialBridgeDefinition = {
  id: string;
  label: string;
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
    label: "Cloudflare",
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
    label: "阿里云 DNS",
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
    label: "DNSPod",
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
    label: "腾讯云 DNS",
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
    id: "godaddy",
    label: "GoDaddy",
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
    label: "Porkbun",
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
    label: "dynv6",
    acmeDnsType: "dns_dynv6",
    ddnsProvider: "dynv6",
    acmeToDdns: [{ from: "DYNV6_TOKEN", to: "token" }],
    ddnsToAcme: [{ from: "token", to: "DYNV6_TOKEN" }],
  },
];

const normalizeValue = (value: string | null | undefined) => value?.trim() || "";

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
}: {
  target: DnsCredentialTarget;
  providerId: string;
  sourceCredentials: Record<string, string>;
  targetCredentials: Record<string, string>;
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
    bridgeLabel: bridge.label,
    fillableFields,
    patch: Object.fromEntries(
      fillableFields.map((field) => [field.targetKey, field.value]),
    ),
  };
};
