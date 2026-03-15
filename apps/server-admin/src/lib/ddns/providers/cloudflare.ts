import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import { getTimeoutMs, parseJsonResponse } from "./helpers";

export const cloudflareProvider: DDNSProviderDefinition = {
  name: "cloudflare",
  label: "Cloudflare",
  fields: [
    { key: "api_token", label: "API 令牌", type: "password", placeholder: "Cloudflare API Token", required: true, description: "需要 Zone.DNS 编辑权限" },
    { key: "zone_id", label: "Zone ID", type: "text", placeholder: "Zone ID", required: true, description: "在 Cloudflare 域名页, 点击三个点, 选择复制区域ID" },
    { key: "domain", label: "域名", type: "text", placeholder: "home.example.com", required: true, description: "要更新的完整域名" },
    { key: "proxied", label: "Cloudflare 代理", type: "select", required: false, options: [{ label: "仅解析", value: "false" }, { label: "橙色云朵", value: "true" }], description: "是否启用 Cloudflare 代理（橙色云朵）" },
  ],
};

export const cloudflareUpdate = async ({ config, http }: DDNSProviderContext, ipv4: string | null, ipv6: string | null): Promise<DDNSUpdateResult> => {
  const { api_token, zone_id, domain, proxied } = config;
  if (!api_token || !zone_id || !domain) {
    return { success: false, message: "Cloudflare 配置不完整" };
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
        errors.push(`查询 A 记录失败: ${JSON.stringify(searchData.errors)}`);
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
            errors.push(`更新 A 记录失败: ${JSON.stringify(updateData.errors)}`);
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
            errors.push(`创建 A 记录失败: ${JSON.stringify(createData.errors)}`);
          }
        }
      }
    } catch (e: any) {
      throw new Error(`A 记录操作异常: ${e?.message || String(e)}`);
    }
  }

  if (ipv6) {
    try {
      const { response: searchRes, data: searchData } = await requestJson(
        `${baseUrl}?type=AAAA&name=${encodeURIComponent(domain)}`,
        { headers },
      );

      if (!searchRes.ok || !searchData.success) {
        errors.push(`查询 AAAA 记录失败: ${JSON.stringify(searchData.errors)}`);
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
            errors.push(`更新 AAAA 记录失败: ${JSON.stringify(updateData.errors)}`);
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
            errors.push(`创建 AAAA 记录失败: ${JSON.stringify(createData.errors)}`);
          }
        }
      }
    } catch (e: any) {
      throw new Error(`AAAA 记录操作异常: ${e?.message || String(e)}`);
    }
  }

  if (errors.length) {
    return { success: false, message: errors.join("; "), ipv4Updated, ipv6Updated };
  }
  return { success: true, message: "Cloudflare DNS 更新成功", ipv4Updated, ipv6Updated };
};
