import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import { getTimeoutMs, parseTextResponse } from "./helpers";

const DUCKDNS_ENDPOINT = "https://ddns.duckdns.fnknock.cn/";

export const duckdnsProvider: DDNSProviderDefinition = {
  name: "duckdns",
  label: "DuckDNS",
  fields: [
    {
      key: "domains",
      label: "子域名",
      type: "text",
      placeholder: "home,lab",
      required: true,
      description: "只填写 DuckDNS 子域名，不带 .duckdns.org 后缀；支持逗号分隔",
    },
    {
      key: "token",
      label: "Token",
      type: "password",
      placeholder: "DuckDNS Token",
      required: true,
      description: "在 DuckDNS 控制台首页可以看到账号 token",
    },
  ],
};

export async function duckdnsUpdate(
  { config, http }: DDNSProviderContext,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const domains = config.domains?.trim();
  const token = config.token?.trim();

  if (!domains || !token) {
    return { success: false, message: "DuckDNS 配置不完整" };
  }

  if (!ipv4 && !ipv6) {
    return { success: false, message: "DuckDNS 更新失败: 没有可用的 IPv4 或 IPv6 地址" };
  }

  const payload = {
    domains,
    token,
    ip: ipv4 || undefined,
    ipv6: ipv6 || undefined,
    verbose: true,
  };
  const timeoutMs = getTimeoutMs();

  try {
    const response = await http.fetch(DUCKDNS_ENDPOINT, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "text/plain",
      },
      body: JSON.stringify(payload),
      signal: AbortSignal.timeout(timeoutMs),
    });

    const text = await parseTextResponse(response);

    if (!response.ok) {
      return { success: false, message: `DuckDNS 更新失败 [${response.status}]: ${text || "请求失败"}` };
    }

    const lines = text.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
    const status = lines[0] || text;

    if (status !== "OK") {
      return { success: false, message: `DuckDNS 更新失败: ${text || "返回了非 OK 响应"}` };
    }

    const result = lines[lines.length - 1];
    const detail = result && result !== "OK" ? ` (${result})` : "";

    return {
      success: true,
      message: `DuckDNS 更新成功${detail}`,
      ipv4Updated: !!ipv4,
      ipv6Updated: !!ipv6,
    };
  } catch (error) {
    const err = error instanceof Error ? error : new Error(String(error));
    throw new Error(`DuckDNS 请求异常: ${err.message}`);
  }
}
