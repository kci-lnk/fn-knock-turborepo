import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import { ddnsProviderT, getTimeoutMs, parseJsonResponse } from "./helpers";

const cloudflareT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("cloudflare", key, params);
const commonT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("common", key, params);

export const cloudflareProvider: DDNSProviderDefinition = {
  name: "cloudflare",
  label: "Cloudflare",
  fields: [
    { key: "api_token", label: cloudflareT("fields.api_token.label"), type: "password", placeholder: "Cloudflare API Token", required: true, description: cloudflareT("fields.api_token.description") },
    { key: "zone_id", label: "Zone ID", type: "text", placeholder: "Zone ID", required: true, description: cloudflareT("fields.zone_id.description") },
    { key: "domain", label: commonT("fields.domain.shortLabel"), type: "text", placeholder: "home.example.com", required: true, description: commonT("fields.domain.description") },
    { key: "proxied", label: cloudflareT("fields.proxied.label"), type: "select", required: false, options: [{ label: cloudflareT("fields.proxied.options.dnsOnly"), value: "false" }, { label: cloudflareT("fields.proxied.options.orangeCloud"), value: "true" }], description: cloudflareT("fields.proxied.description") },
  ],
};

export const cloudflareUpdate = async ({ config, http }: DDNSProviderContext, ipv4: string | null, ipv6: string | null): Promise<DDNSUpdateResult> => {
  const { api_token, zone_id, domain, proxied } = config;
  if (!api_token || !zone_id || !domain) {
    return { success: false, message: cloudflareT("configIncomplete") };
  }

  const isProxied = proxied === "true";
  const baseUrl = `https://api.cloudflare.com/client/v4/zones/${zone_id}/dns_records`;
  const headers = {
    Authorization: `Bearer ${api_token}`,
    "Content-Type": "application/json",
  };

  let ipv4Updated = false;
  let ipv6Updated = false;
  const errors: string[] = [];
  const requestJson = async (url: string, init?: RequestInit) => {
    const response = await http.fetch(url, {
      ...init,
      signal: AbortSignal.timeout(getTimeoutMs()),
    });
    const data = await parseJsonResponse<any>(response);
    return { response, data };
  };

  if (ipv4) {
    try {
      const { response: searchRes, data: searchData } = await requestJson(
        `${baseUrl}?type=A&name=${encodeURIComponent(domain)}`,
        { headers },
      );

      if (!searchRes.ok || !searchData.success) {
        errors.push(cloudflareT("searchRecordFailed", { type: "A", detail: JSON.stringify(searchData.errors) }));
      } else {
        const existing = searchData.result?.[0];
        if (existing) {
          const { response: updateRes, data: updateData } = await requestJson(`${baseUrl}/${existing.id}`, {
            method: "PATCH",
            headers,
            body: JSON.stringify({ type: "A", name: domain, content: ipv4, proxied: isProxied }),
          });
          if (updateRes.ok && updateData.success) {
            ipv4Updated = true;
          } else {
            errors.push(cloudflareT("updateRecordFailed", { type: "A", detail: JSON.stringify(updateData.errors) }));
          }
        } else {
          const { response: createRes, data: createData } = await requestJson(baseUrl, {
            method: "POST",
            headers,
            body: JSON.stringify({ type: "A", name: domain, content: ipv4, proxied: isProxied, ttl: 1 }),
          });
          if (createRes.ok && createData.success) {
            ipv4Updated = true;
          } else {
            errors.push(cloudflareT("createRecordFailed", { type: "A", detail: JSON.stringify(createData.errors) }));
          }
        }
      }
    } catch (e: any) {
      throw new Error(cloudflareT("recordOperationError", { type: "A", detail: e?.message || String(e) }));
    }
  }

  if (ipv6) {
    try {
      const { response: searchRes, data: searchData } = await requestJson(
        `${baseUrl}?type=AAAA&name=${encodeURIComponent(domain)}`,
        { headers },
      );

      if (!searchRes.ok || !searchData.success) {
        errors.push(cloudflareT("searchRecordFailed", { type: "AAAA", detail: JSON.stringify(searchData.errors) }));
      } else {
        const existing = searchData.result?.[0];
        if (existing) {
          const { response: updateRes, data: updateData } = await requestJson(`${baseUrl}/${existing.id}`, {
            method: "PATCH",
            headers,
            body: JSON.stringify({ type: "AAAA", name: domain, content: ipv6, proxied: isProxied }),
          });
          if (updateRes.ok && updateData.success) {
            ipv6Updated = true;
          } else {
            errors.push(cloudflareT("updateRecordFailed", { type: "AAAA", detail: JSON.stringify(updateData.errors) }));
          }
        } else {
          const { response: createRes, data: createData } = await requestJson(baseUrl, {
            method: "POST",
            headers,
            body: JSON.stringify({ type: "AAAA", name: domain, content: ipv6, proxied: isProxied, ttl: 1 }),
          });
          if (createRes.ok && createData.success) {
            ipv6Updated = true;
          } else {
            errors.push(cloudflareT("createRecordFailed", { type: "AAAA", detail: JSON.stringify(createData.errors) }));
          }
        }
      }
    } catch (e: any) {
      throw new Error(cloudflareT("recordOperationError", { type: "AAAA", detail: e?.message || String(e) }));
    }
  }

  if (errors.length) {
    return { success: false, message: errors.join("; "), ipv4Updated, ipv6Updated };
  }
  return { success: true, message: cloudflareT("success"), ipv4Updated, ipv6Updated };
};
