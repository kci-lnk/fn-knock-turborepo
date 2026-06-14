import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import { ddnsProviderT, getTimeoutMs } from "./helpers";

const dynv6T = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("dynv6", key, params);

export const dynv6Provider: DDNSProviderDefinition = {
  name: "dynv6",
  label: "dynv6",
  fields: [
    { key: "token", label: "HTTP Token", type: "password", placeholder: "dynv6 HTTP Token", required: true, description: dynv6T("fields.token.description") },
    { key: "zone", label: dynv6T("fields.zone.label"), type: "text", placeholder: "myhost.dynv6.net", required: true, description: dynv6T("fields.zone.description") },
    { key: "ipv6prefix", label: "IPv6 Prefix", type: "text", placeholder: "2001:db8:1234::/64", required: false, description: dynv6T("fields.ipv6prefix.description") },
  ],
};

export const dynv6Update = async ({ config, http }: DDNSProviderContext, ipv4: string | null, ipv6: string | null): Promise<DDNSUpdateResult> => {
  const { token, zone, ipv6prefix } = config;
  if (!token || !zone) {
    return { success: false, message: dynv6T("configIncomplete") };
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
    const emptyValue = dynv6T("empty");
    const sentParams = `ipv4=${ipv4 || emptyValue}, ipv6=${ipv6 || emptyValue}${ipv6prefix ? `, ipv6prefix=${ipv6prefix}` : ""}`;

    if (res.ok && (text.includes("updated") || text.includes("unchanged"))) {
      return {
        success: true,
        message: dynv6T("success", { detail: text, params: sentParams }),
        ipv4Updated: !!ipv4,
        ipv6Updated: !!ipv6,
      };
    }

    return {
      success: false,
      message: dynv6T("updateFailed", { status: res.status, detail: text }),
    };
  } catch (e: any) {
    throw new Error(dynv6T("requestError", { detail: e?.message || String(e) }));
  }
};
