import { type AcmeDnsProvider } from "@/lib/api/acme";

export type DnsCredentialScheme = AcmeDnsProvider["credentialSchemes"][number];
export type DnsCredentialField = DnsCredentialScheme["fields"][number];

export const getProviderCredentialFields = (
  provider: AcmeDnsProvider | null,
) => {
  if (!provider) return [] as DnsCredentialField[];

  const fields: DnsCredentialField[] = [];
  const seen = new Set<string>();

  for (const scheme of provider.credentialSchemes) {
    for (const field of scheme.fields) {
      if (seen.has(field.key)) continue;
      seen.add(field.key);
      fields.push(field);
    }
  }

  return fields;
};

export const getSatisfiedCredentialScheme = (
  provider: AcmeDnsProvider | null,
  values: Record<string, string>,
) => {
  if (!provider) return null;

  return (
    provider.credentialSchemes.find((scheme) =>
      scheme.fields
        .filter((field) => field.required !== false)
        .every((field) => Boolean((values[field.key] || "").trim())),
    ) || null
  );
};

export const buildAcmeCredentialsPayload = (
  credentials: Record<string, string>,
) => {
  const payload: Record<string, string> = {};
  for (const [key, value] of Object.entries(credentials || {})) {
    const normalizedKey = key.trim();
    const normalizedValue = String(value ?? "").trim();
    if (!normalizedKey || !normalizedValue) continue;
    payload[normalizedKey] = normalizedValue;
  }
  return payload;
};

export const normalizeProviderCredentials = (
  provider: AcmeDnsProvider | null,
  credentials: Record<string, string>,
) => {
  const normalized: Record<string, string> = {};
  for (const field of getProviderCredentialFields(provider)) {
    const value = credentials[field.key];
    normalized[field.key] = typeof value === "string" ? value : "";
  }
  return normalized;
};

export const getProviderGroupKey = (group?: string | null) => {
  if (group === "\u5e38\u7528") return "common";
  if (group === "\u56fd\u5185") return "china";
  if (group === "\u56fd\u9645") return "international";
  if (group === "\u81ea\u5efa/\u9ad8\u7ea7") return "customAdvanced";
  if (!group || group === "\u5176\u4ed6") return "other";
  return group;
};
