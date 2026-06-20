import {
  dnsProviders,
  filterAcmeCredentialsForProvider,
  formatCredentialRequirements,
  getSatisfiedCredentialScheme,
  normalizeAcmeDnsType,
} from "../../lib/acme-dns-providers";

type TranslationParams = Record<
  string,
  string | number | boolean | null | undefined
>;

type Translator = (key: string, params?: TranslationParams) => string;

const normalizeDomains = (domains: string[]) => {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const raw of domains || []) {
    const v = String(raw ?? "")
      .trim()
      .toLowerCase();
    if (!v) continue;
    if (!isValidDomain(v)) continue;
    if (seen.has(v)) continue;
    seen.add(v);
    out.push(v);
  }
  return out;
};

const isValidDomain = (value: string) => {
  if (!value) return false;
  if (value.length > 253) return false;
  const v = value.trim();
  if (!v) return false;
  if (v.includes("..")) return false;
  if (v.startsWith(".") || v.endsWith(".")) return false;
  if (v.includes("/") || v.includes(" ") || v.includes("\t")) return false;
  return /^(\*\.)?([a-z0-9-]+\.)+[a-z0-9-]+$/i.test(v);
};

export const validateAndNormalizeAcmeRequest = (
  input: {
    domains: string[];
    dnsType?: string;
    provider?: string;
    credentials?: Record<string, string>;
  },
  routeT: Translator,
  providerT: Translator,
) => {
  const domains = normalizeDomains(input.domains);
  if (domains.length === 0) throw new Error(routeT("domainsInvalid"));

  const dnsType = normalizeAcmeDnsType(input.dnsType ?? input.provider);
  if (!dnsType) throw new Error(routeT("dnsTypeRequired"));
  const provider = dnsProviders.find((p) => p.dnsType === dnsType) || null;
  if (!provider) throw new Error(routeT("unsupportedDnsProvider"));

  const credentials = filterAcmeCredentialsForProvider(
    provider,
    input.credentials,
  );
  const matchedScheme = getSatisfiedCredentialScheme(provider, credentials);
  if (!matchedScheme) {
    throw new Error(
      routeT("missingDnsCredentials", {
        requirements: formatCredentialRequirements(provider, providerT),
      }),
    );
  }

  return { domains, dnsType, credentials };
};
