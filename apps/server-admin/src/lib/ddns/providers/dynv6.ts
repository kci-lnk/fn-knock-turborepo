import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import { getTimeoutMs } from "./helpers";

export const dynv6Provider: DDNSProviderDefinition = {
  name: "dynv6",
  label: "dynv6",
  fields: [
    { key: "token", label: "HTTP Token", type: "password", placeholder: "dynv6 HTTP Token", required: true, description: "在 dynv6.com 账户中生成" },
    { key: "zone", label: "Zone 名称", type: "text", placeholder: "myhost.dynv6.net", required: true, description: "你的 dynv6 zone 域名" },
    { key: "ipv6prefix", label: "IPv6 Prefix", type: "text", placeholder: "2001:db8:1234::/64", required: false, description: "可选，透传给 dynv6 API" },
  ],
};

export const dynv6Update = async ({ config, http }: DDNSProviderContext, ipv4: string | null, ipv6: string | null): Promise<DDNSUpdateResult> => {
  const { token, zone, ipv6prefix } = config;
  if (!token || !zone) {
    return { success: false, message: "dynv6 配置不完整" };
  }

  const params = new URLSearchParams({ hostname: zone, token });
  if (ipv4) params.set("ipv4", ipv4);
  if (ipv6) params.set("ipv6", ipv6);
  if (ipv6prefix) params.set("ipv6prefix", ipv6prefix);

  const url = `https://dynv6.com/api/update?${params.toString()}`;
  const timeoutMs = getTimeoutMs();

  try {
    const res = await http.fetch(url, {
      signal: AbortSignal.timeout(timeoutMs),
    });

    const text = (await res.text()).trim();
    const sentParams = `ipv4=${ipv4 || "(空)"}, ipv6=${ipv6 || "(空)"}${ipv6prefix ? `, ipv6prefix=${ipv6prefix}` : ""}`;

    if (res.ok && (text.includes("updated") || text.includes("unchanged"))) {
      return {
        success: true,
        message: `dynv6: ${text} (发送: ${sentParams})`,
        ipv4Updated: !!ipv4,
        ipv6Updated: !!ipv6,
      };
    }

    return { success: false, message: `dynv6 更新失败 [${res.status}]: ${text}` };
  } catch (e: any) {
    throw new Error(`dynv6 请求异常: ${e?.message || String(e)}`);
  }
};
