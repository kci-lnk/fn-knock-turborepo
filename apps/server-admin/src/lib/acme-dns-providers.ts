import { tDefault } from "./i18n";

export type DnsProvider = {
  dnsType: string;
  label: string;
  group: string;
  credentialSchemes: DnsCredentialScheme[];
};

export type DnsCredentialField = {
  key: string;
  label?: string;
  description?: string;
  required?: boolean;
};

export type DnsCredentialScheme = {
  id: string;
  label: string;
  description?: string;
  fields: DnsCredentialField[];
};

type CredentialSchemeOptions = {
  description?: string;
  optionalKeys?: string[];
  fields?: Partial<Record<string, Omit<DnsCredentialField, "key">>>;
  defaultCredentialLabel?: string;
};

type AcmeDnsProviderTranslator = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => string;

const acmeDnsProviderT = (
  translator: AcmeDnsProviderTranslator,
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => translator(`server.acmeDnsProviders.${key}`, params);

const createCredentialScheme = (
  id: string,
  label: string,
  fieldKeys: string[],
  options?: CredentialSchemeOptions,
): DnsCredentialScheme => {
  const optionalKeys = new Set(options?.optionalKeys || []);

  return {
    id,
    label,
    description: options?.description,
    fields: fieldKeys.map((key) => ({
      key,
      required: !optionalKeys.has(key),
      ...(options?.fields?.[key] || {}),
    })),
  };
};

const createSingleSchemeProviderBase = (
  dnsType: string,
  label: string,
  group: string,
  envKeys: string[],
  options?: CredentialSchemeOptions,
): DnsProvider => ({
  dnsType,
  label,
  group,
  credentialSchemes: [
    createCredentialScheme(
      "default",
      options?.defaultCredentialLabel || "Default credentials",
      envKeys,
      options,
    ),
  ],
});

export const createAcmeDnsProviders = (
  translator: AcmeDnsProviderTranslator = tDefault,
): DnsProvider[] => {
  const text = (
    key: string,
    params?: Record<string, string | number | boolean | null | undefined>,
  ) => acmeDnsProviderT(translator, key, params);
  const groups = {
    common: text("groups.common"),
    domestic: text("groups.domestic"),
    international: text("groups.international"),
    selfHostedAdvanced: text("groups.selfHostedAdvanced"),
  };
  const createSingleSchemeProvider = (
    dnsType: string,
    label: string,
    group: string,
    envKeys: string[],
    options?: CredentialSchemeOptions,
  ): DnsProvider =>
    createSingleSchemeProviderBase(dnsType, label, group, envKeys, {
      ...options,
      defaultCredentialLabel: text("credentialSchemes.default"),
    });

  return [
  {
    dnsType: "dns_cf",
    label: "Cloudflare",
    group: groups.common,
    credentialSchemes: [
      createCredentialScheme(
        "global-key",
        "Global API Key",
        ["CF_Key", "CF_Email"],
        {
          description: text("cloudflare.globalKeyDescription"),
          fields: {
            CF_Key: { label: "Global API Key" },
            CF_Email: { label: text("fields.accountEmail") },
          },
        },
      ),
      createCredentialScheme(
        "api-token",
        "API Token",
        ["CF_Token", "CF_Zone_ID", "CF_Account_ID"],
        {
          description: text("cloudflare.apiTokenDescription"),
          optionalKeys: ["CF_Zone_ID", "CF_Account_ID"],
          fields: {
            CF_Token: { label: "API Token" },
            CF_Zone_ID: { label: "Zone ID" },
            CF_Account_ID: { label: "Account ID" },
          },
        },
      ),
    ],
  },
  createSingleSchemeProvider("dns_ali", text("labels.aliyun"), groups.common, [
    "Ali_Key",
    "Ali_Secret",
  ]),
  createSingleSchemeProvider("dns_dp", "DNSPod", groups.common, [
    "DP_Id",
    "DP_Key",
  ]),
  createSingleSchemeProvider(
    "dns_tencent",
    text("labels.tencentCloudDnspod"),
    groups.common,
    ["Tencent_SecretId", "Tencent_SecretKey"],
  ),
  createSingleSchemeProvider("dns_duckdns", "DuckDNS", groups.common, [
    "DuckDNS_Token",
  ]),
  createSingleSchemeProvider("dns_gd", "GoDaddy", groups.common, [
    "GD_Key",
    "GD_Secret",
  ]),
  createSingleSchemeProvider("dns_dgon", "DigitalOcean", groups.common, [
    "DO_API_KEY",
  ]),
  createSingleSchemeProvider("dns_netlify", "Netlify", groups.common, [
    "NETLIFY_ACCESS_TOKEN",
  ]),
  createSingleSchemeProvider("dns_vercel", "Vercel", groups.common, [
    "VERCEL_TOKEN",
  ]),
  createSingleSchemeProvider("dns_aws", "AWS Route53", groups.common, [
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
  ]),
  createSingleSchemeProvider(
    "dns_gcloud",
    "Google Cloud DNS (gcloud)",
    groups.common,
    ["CLOUDSDK_ACTIVE_CONFIG_NAME"],
    {
      description: text("gcloud.description"),
      optionalKeys: ["CLOUDSDK_ACTIVE_CONFIG_NAME"],
    },
  ),
  {
    dnsType: "dns_azure",
    label: "Azure DNS",
    group: groups.common,
    credentialSchemes: [
      createCredentialScheme("service-principal", "Service Principal", [
        "AZUREDNS_SUBSCRIPTIONID",
        "AZUREDNS_TENANTID",
        "AZUREDNS_APPID",
        "AZUREDNS_CLIENTSECRET",
      ]),
      createCredentialScheme("bearer-token", "Bearer Token", [
        "AZUREDNS_SUBSCRIPTIONID",
        "AZUREDNS_BEARERTOKEN",
      ]),
      createCredentialScheme(
        "managed-identity",
        "Managed Identity",
        ["AZUREDNS_SUBSCRIPTIONID", "AZUREDNS_MANAGEDIDENTITY"],
        {
          description: text("azure.managedIdentityDescription"),
        },
      ),
    ],
  },
  createSingleSchemeProvider("dns_porkbun", "Porkbun", groups.common, [
    "PORKBUN_API_KEY",
    "PORKBUN_SECRET_API_KEY",
  ]),
  {
    dnsType: "dns_dynv6",
    label: "dynv6",
    group: groups.common,
    credentialSchemes: [
      createCredentialScheme("rest-token", "REST API Token", ["DYNV6_TOKEN"]),
      createCredentialScheme("ssh-key", "SSH Key", ["KEY"], {
        fields: {
          KEY: { label: text("fields.sshPrivateKeyPath") },
        },
      }),
    ],
  },
  createSingleSchemeProvider(
    "dns_huaweicloud",
    text("labels.huaweiCloudDns"),
    groups.domestic,
    [
      "HUAWEICLOUD_Username",
      "HUAWEICLOUD_Password",
      "HUAWEICLOUD_DomainName",
    ],
  ),
  createSingleSchemeProvider("dns_jd", text("labels.jdCloudDns"), groups.domestic, [
    "JD_ACCESS_KEY_ID",
    "JD_ACCESS_KEY_SECRET",
    "JD_REGION",
  ]),
  createSingleSchemeProvider("dns_la", "DNS.LA", groups.domestic, ["LA_Id", "LA_Sk"]),
  createSingleSchemeProvider("dns_west_cn", text("labels.westCn"), groups.domestic, [
    "WEST_Username",
    "WEST_Key",
  ]),
  createSingleSchemeProvider("dns_linode_v4", "Linode", groups.international, [
    "LINODE_V4_API_KEY",
  ]),
  createSingleSchemeProvider("dns_vultr", "Vultr", groups.international, ["VULTR_API_KEY"]),
  createSingleSchemeProvider(
    "dns_ovh",
    "OVH",
    groups.international,
    ["OVH_AK", "OVH_AS", "OVH_CK", "OVH_END_POINT"],
    {
      optionalKeys: ["OVH_END_POINT"],
    },
  ),
  createSingleSchemeProvider("dns_hetzner", "Hetzner", groups.international, [
    "HETZNER_Token",
  ]),
  createSingleSchemeProvider("dns_namecheap", "Namecheap", groups.international, [
    "NAMECHEAP_API_KEY",
    "NAMECHEAP_USERNAME",
    "NAMECHEAP_SOURCEIP",
  ]),
  createSingleSchemeProvider("dns_namecom", "Name.com", groups.international, [
    "Namecom_Username",
    "Namecom_Token",
  ]),
  createSingleSchemeProvider("dns_namesilo", "NameSilo", groups.international, [
    "Namesilo_Key",
  ]),
  createSingleSchemeProvider("dns_dreamhost", "DreamHost", groups.international, [
    "DH_API_KEY",
  ]),
  createSingleSchemeProvider("dns_freedns", "FreeDNS", groups.international, [
    "FREEDNS_User",
    "FREEDNS_Password",
  ]),
  createSingleSchemeProvider("dns_dyn", "Dyn Managed DNS", groups.international, [
    "DYN_Customer",
    "DYN_Username",
    "DYN_Password",
  ]),
  createSingleSchemeProvider("dns_dynu", "Dynu", groups.international, [
    "Dynu_ClientId",
    "Dynu_Secret",
  ]),
  createSingleSchemeProvider("dns_bunny", "Bunny DNS", groups.international, [
    "BUNNY_API_KEY",
  ]),
  createSingleSchemeProvider("dns_desec", "deSEC", groups.international, ["DEDYN_TOKEN"]),
  createSingleSchemeProvider("dns_freemyip", "FreeMyIP", groups.international, [
    "FREEMYIP_Token",
  ]),
  createSingleSchemeProvider("dns_ipv64", "IPv64.net", groups.international, ["IPv64_Token"]),
  createSingleSchemeProvider("dns_scaleway", "Scaleway", groups.international, [
    "SCALEWAY_API_TOKEN",
  ]),
  createSingleSchemeProvider("dns_easydns", "easyDNS", groups.international, [
    "EASYDNS_Token",
    "EASYDNS_Key",
  ]),
  createSingleSchemeProvider("dns_zoneedit", "ZoneEdit", groups.international, [
    "ZONEEDIT_ID",
    "ZONEEDIT_Token",
  ]),
  createSingleSchemeProvider("dns_zonomi", "Zonomi", groups.international, ["ZM_Key"]),
  createSingleSchemeProvider("dns_dnsexit", "DNSExit", groups.international, [
    "DNSEXIT_API_KEY",
    "DNSEXIT_AUTH_USER",
    "DNSEXIT_AUTH_PASS",
  ]),
  {
    dnsType: "dns_yandex360",
    label: "Yandex 360",
    group: groups.international,
    credentialSchemes: [
      createCredentialScheme(
        "oauth-client",
        "OAuth Client",
        ["YANDEX360_CLIENT_ID", "YANDEX360_CLIENT_SECRET", "YANDEX360_ORG_ID"],
        {
          optionalKeys: ["YANDEX360_ORG_ID"],
        },
      ),
      createCredentialScheme(
        "access-token",
        "Access Token",
        ["YANDEX360_ACCESS_TOKEN", "YANDEX360_ORG_ID"],
        {
          optionalKeys: ["YANDEX360_ORG_ID"],
        },
      ),
    ],
  },
  createSingleSchemeProvider("dns_mydnsjp", "MyDNS.JP", groups.international, [
    "MYDNSJP_MasterID",
    "MYDNSJP_Password",
  ]),
  createSingleSchemeProvider("dns_gandi_livedns", "Gandi LiveDNS", groups.international, [
    "GANDI_LIVEDNS_KEY",
  ]),
  createSingleSchemeProvider("dns_nsone", "NS1", groups.international, ["NS1_Key"]),
  createSingleSchemeProvider("dns_dnsimple", "DNSimple", groups.international, [
    "DNSimple_OAUTH_TOKEN",
  ]),
  {
    dnsType: "dns_cloudns",
    label: "ClouDNS",
    group: groups.international,
    credentialSchemes: [
      createCredentialScheme("auth-id", "Auth ID", [
        "CLOUDNS_AUTH_ID",
        "CLOUDNS_AUTH_PASSWORD",
      ]),
      createCredentialScheme("sub-auth-id", "Sub Auth ID", [
        "CLOUDNS_SUB_AUTH_ID",
        "CLOUDNS_AUTH_PASSWORD",
      ]),
    ],
  },
  createSingleSchemeProvider("dns_he", "Hurricane Electric", groups.international, [
    "HE_Username",
    "HE_Password",
  ]),
  createSingleSchemeProvider("dns_transip", "TransIP", groups.international, [
    "TRANSIP_Username",
    "TRANSIP_Key_File",
  ]),
  createSingleSchemeProvider("dns_doapi", "Domain-Offensive", groups.international, [
    "DO_LETOKEN",
  ]),
  createSingleSchemeProvider(
    "dns_acmedns",
    "acme-dns",
    groups.selfHostedAdvanced,
    [
      "ACMEDNS_USERNAME",
      "ACMEDNS_PASSWORD",
      "ACMEDNS_SUBDOMAIN",
      "ACMEDNS_BASE_URL",
    ],
    {
      optionalKeys: ["ACMEDNS_BASE_URL"],
    },
  ),
  createSingleSchemeProvider(
    "dns_nsupdate",
    "nsupdate",
    groups.selfHostedAdvanced,
    [
      "NSUPDATE_SERVER",
      "NSUPDATE_SERVER_PORT",
      "NSUPDATE_KEY",
      "NSUPDATE_ZONE",
    ],
    {
      optionalKeys: ["NSUPDATE_SERVER_PORT", "NSUPDATE_KEY", "NSUPDATE_ZONE"],
    },
  ),
  createSingleSchemeProvider(
    "dns_pdns",
    "PowerDNS",
    groups.selfHostedAdvanced,
    ["PDNS_Url", "PDNS_ServerId", "PDNS_Token", "PDNS_Ttl"],
    {
      optionalKeys: ["PDNS_Ttl"],
    },
  ),
  createSingleSchemeProvider(
    "dns_technitium",
    "Technitium DNS",
    groups.selfHostedAdvanced,
    ["Technitium_Server", "Technitium_Token", "Technitium_Expiry_Ttl"],
    {
      optionalKeys: ["Technitium_Expiry_Ttl"],
    },
  ),
  createSingleSchemeProvider("dns_pleskxml", "Plesk XML API", groups.selfHostedAdvanced, [
    "pleskxml_uri",
    "pleskxml_user",
    "pleskxml_pass",
  ]),
  createSingleSchemeProvider("dns_cpanel", "cPanel", groups.selfHostedAdvanced, [
    "cPanel_Username",
    "cPanel_Apitoken",
    "cPanel_Hostname",
  ]),
  createSingleSchemeProvider(
    "dns_da",
    "DirectAdmin",
    groups.selfHostedAdvanced,
    ["DA_Api", "DA_Api_Insecure"],
    {
      fields: {
        DA_Api_Insecure: { description: text("descriptions.boolean01") },
      },
    },
  ),
  createSingleSchemeProvider(
    "dns_ispconfig",
    "ISPConfig",
    groups.selfHostedAdvanced,
    ["ISPC_User", "ISPC_Password", "ISPC_Api", "ISPC_Api_Insecure"],
    {
      fields: {
        ISPC_Api_Insecure: { description: text("descriptions.boolean01") },
      },
    },
  ),
  createSingleSchemeProvider(
    "dns_opnsense",
    "OPNsense",
    groups.selfHostedAdvanced,
    ["OPNs_Host", "OPNs_Port", "OPNs_Key", "OPNs_Token", "OPNs_Api_Insecure"],
    {
      optionalKeys: ["OPNs_Port", "OPNs_Api_Insecure"],
      fields: {
        OPNs_Api_Insecure: {
          description: text("descriptions.optionalBoolean01"),
        },
      },
    },
  ),
  ];
};

export const dnsProviders: DnsProvider[] = createAcmeDnsProviders();

const dnsTypeAliases: Record<string, string> = {
  aliyun: "dns_ali",
  cloudflare: "dns_cf",
  dnspod: "dns_dp",
  tencentcloud: "dns_tencent",
  duckdns: "dns_duckdns",
  google: "dns_gcloud",
  gcloud: "dns_gcloud",
  dns_google: "dns_gcloud",
  huaweicloud: "dns_huaweicloud",
  huawei: "dns_huaweicloud",
  netlify: "dns_netlify",
};

const credentialAliases: Record<string, Record<string, string>> = {
  dns_netlify: {
    NETLIFY_TOKEN: "NETLIFY_ACCESS_TOKEN",
  },
};

const normalizeCredentialRecord = (value: unknown) => {
  const out: Record<string, string> = {};
  if (!value || typeof value !== "object") return out;
  for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
    const kk = String(k ?? "").trim();
    const vv = String(v ?? "").trim();
    if (!kk || !vv) continue;
    out[kk] = vv;
  }
  return out;
};

export const normalizeAcmeDnsType = (value: string | undefined | null) => {
  if (!value) return null;
  const v = value.trim();
  if (!v) return null;

  const lower = v.toLowerCase();
  if (dnsTypeAliases[lower]) return dnsTypeAliases[lower];
  if (/^dns_[a-z0-9_]+$/i.test(v)) return lower;
  return null;
};

export const getProviderAllCredentialKeys = (provider: DnsProvider) => {
  const keys: string[] = [];
  const seen = new Set<string>();
  for (const scheme of provider.credentialSchemes) {
    for (const field of scheme.fields) {
      if (seen.has(field.key)) continue;
      seen.add(field.key);
      keys.push(field.key);
    }
  }
  return keys;
};

export const getSatisfiedCredentialScheme = (
  provider: DnsProvider,
  credentials: Record<string, string>,
) => {
  return (
    provider.credentialSchemes.find((scheme) =>
      scheme.fields
        .filter((field) => field.required !== false)
        .every((field) => Boolean(credentials[field.key])),
    ) || null
  );
};

export const formatCredentialRequirements = (
  provider: DnsProvider,
  translator: AcmeDnsProviderTranslator = tDefault,
) => {
  const text = (
    key: string,
    params?: Record<string, string | number | boolean | null | undefined>,
  ) => acmeDnsProviderT(translator, key, params);
  if (provider.credentialSchemes.length === 1) {
    const requiredKeys = provider.credentialSchemes[0]!.fields.filter(
      (field) => field.required !== false,
    ).map((field) => field.key);
    return requiredKeys.join(", ");
  }

  return provider.credentialSchemes
    .map((scheme) => {
      const requiredKeys = scheme.fields
        .filter((field) => field.required !== false)
        .map((field) => field.key)
        .join(", ");
      const optionalKeys = scheme.fields
        .filter((field) => field.required === false)
        .map((field) => field.key);
      const suffix = optionalKeys.length
        ? text("requirements.optionalSuffix", {
            keys: optionalKeys.join(", "),
          })
        : "";
      return `${scheme.label}: ${requiredKeys}${suffix}`;
    })
    .join(text("requirements.orSeparator"));
};

export const normalizeAcmeEnvVars = (
  dnsType: string | undefined | null,
  credentials: Record<string, string> | undefined | null,
) => {
  const normalized = normalizeCredentialRecord(credentials);
  const normalizedDnsType = normalizeAcmeDnsType(dnsType) || "";
  const aliases = credentialAliases[normalizedDnsType] || {};

  for (const [from, to] of Object.entries(aliases)) {
    if (!normalized[to] && normalized[from]) {
      normalized[to] = normalized[from];
    }
  }

  return normalized;
};

export const filterAcmeCredentialsForProvider = (
  provider: DnsProvider,
  credentials: Record<string, string> | undefined | null,
) => {
  const allowedCredentialKeys = new Set(getProviderAllCredentialKeys(provider));
  return Object.fromEntries(
    Object.entries(normalizeAcmeEnvVars(provider.dnsType, credentials)).filter(
      ([key]) => allowedCredentialKeys.has(key),
    ),
  );
};

export const getProviderLabel = (
  dnsType: string | null | undefined,
  translator: AcmeDnsProviderTranslator = tDefault,
) => {
  const normalized =
    normalizeAcmeDnsType(dnsType) || String(dnsType || "").trim();
  if (!normalized) return "-";
  return (
    createAcmeDnsProviders(translator).find(
      (provider) => provider.dnsType === normalized,
    )?.label ||
    normalized
  );
};
