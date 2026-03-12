import type { DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import { getTimeoutMs, splitDomain, toPositiveInt, updateDualStack } from "./helpers";

export const godaddyProvider: DDNSProviderDefinition = {
  name: "godaddy",
  label: "GoDaddy",
  fields: [
    { key: "api_key", label: "API Key", type: "text", placeholder: "GoDaddy API Key", required: true },
    { key: "api_secret", label: "API Secret", type: "password", placeholder: "GoDaddy API Secret", required: true },
    { key: "root_domain", label: "根域名", type: "text", placeholder: "example.com", required: true },
    { key: "domain", label: "完整域名", type: "text", placeholder: "home.example.com", required: true },
    { key: "ttl", label: "TTL", type: "text", placeholder: "600", required: false, description: "默认 600 秒" },
  ],
};

export async function godaddyUpdate(
  config: Record<string, string>,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const { api_key, api_secret, root_domain, domain } = config;
  if (!api_key || !api_secret || !root_domain || !domain) {
    return { success: false, message: "GoDaddy 配置不完整" };
  }

  const ttl = toPositiveInt(config.ttl, 600);
  const parsed = splitDomain(domain, root_domain);

  return updateDualStack("GoDaddy", ipv4, ipv6, async (recordType, ip) => {
    const response = await fetch(
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
      throw new Error(`[${response.status}] ${text || "更新失败"}`);
    }
  });
}
