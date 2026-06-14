import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import { ddnsProviderT, getTimeoutMs, parseTextResponse } from "./helpers";

const DUCKDNS_ENDPOINT = "https://ddns.duckdns.fnknock.cn/";
const duckdnsT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("duckdns", key, params);

export const duckdnsProvider: DDNSProviderDefinition = {
  name: "duckdns",
  label: "DuckDNS",
  fields: [
    {
      key: "domains",
      label: duckdnsT("fields.domains.label"),
      type: "text",
      placeholder: "home,lab",
      required: true,
      description: duckdnsT("fields.domains.description"),
    },
    {
      key: "token",
      label: "Token",
      type: "password",
      placeholder: "DuckDNS Token",
      required: true,
      description: duckdnsT("fields.token.description"),
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
    return { success: false, message: duckdnsT("configIncomplete") };
  }

  if (!ipv4 && !ipv6) {
    return { success: false, message: duckdnsT("noIpAvailable") };
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
      return {
        success: false,
        message: duckdnsT("updateFailedWithStatus", {
          status: response.status,
          detail: text || duckdnsT("requestFailed"),
        }),
      };
    }

    const lines = text.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
    const status = lines[0] || text;

    if (status !== "OK") {
      return {
        success: false,
        message: duckdnsT("updateFailed", {
          detail: text || duckdnsT("nonOkResponse"),
        }),
      };
    }

    const result = lines[lines.length - 1];
    const detail = result && result !== "OK" ? ` (${result})` : "";

    return {
      success: true,
      message: duckdnsT("success", { detail }),
      ipv4Updated: !!ipv4,
      ipv6Updated: !!ipv6,
    };
  } catch (error) {
    const err = error instanceof Error ? error : new Error(String(error));
    throw new Error(duckdnsT("requestError", { detail: err.message }));
  }
}
