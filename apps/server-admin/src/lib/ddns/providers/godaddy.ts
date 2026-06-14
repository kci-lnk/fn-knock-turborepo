import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import {
  ddnsProviderT,
  getTimeoutMs,
  splitDomain,
  toPositiveInt,
  updateDualStack,
} from "./helpers";

const godaddyT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("godaddy", key, params);
const commonT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("common", key, params);

export const godaddyProvider: DDNSProviderDefinition = {
  name: "godaddy",
  label: "GoDaddy",
  fields: [
    { key: "api_key", label: "API Key", type: "text", placeholder: "GoDaddy API Key", required: true },
    { key: "api_secret", label: "API Secret", type: "password", placeholder: "GoDaddy API Secret", required: true },
    { key: "root_domain", label: commonT("fields.root_domain.label"), type: "text", placeholder: "example.com", required: true },
    { key: "domain", label: commonT("fields.domain.label"), type: "text", placeholder: "home.example.com", required: true },
    { key: "ttl", label: "TTL", type: "text", placeholder: "600", required: false, description: commonT("fields.ttl.description", { seconds: 600 }) },
  ],
};

export async function godaddyUpdate(
  { config, http }: DDNSProviderContext,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const { api_key, api_secret, root_domain, domain } = config;
  if (!api_key || !api_secret || !root_domain || !domain) {
    return { success: false, message: godaddyT("configIncomplete") };
  }

  const ttl = toPositiveInt(config.ttl, 600);
  const parsed = splitDomain(domain, root_domain);

  return updateDualStack("GoDaddy", ipv4, ipv6, async (recordType, ip) => {
    const response = await http.fetch(
      `https://api.godaddy.com/v1/domains/${encodeURIComponent(parsed.rootDomain)}/records/${recordType}/${encodeURIComponent(parsed.recordName)}`,
      {
        method: "PUT",
        headers: {
          Authorization: `sso-key ${api_key}:${api_secret}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify([
          {
            data: ip,
            name: parsed.recordName,
            ttl,
            type: recordType,
          },
        ]),
        signal: AbortSignal.timeout(getTimeoutMs()),
      },
    );

    if (!response.ok) {
      const text = await response.text();
      throw new Error(
        godaddyT("updateFailedWithStatus", {
          status: response.status,
          detail: text || godaddyT("updateFailed"),
        }),
      );
    }
  });
}
