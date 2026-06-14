import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import {
  ddnsProviderT,
  getTimeoutMs,
  parseJsonResponse,
  splitDomain,
  toPositiveInt,
  updateDualStack,
} from "./helpers";

const PORKBUN_ENDPOINT = "https://porkbun.com/api/json/v3/dns";
const porkbunT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("porkbun", key, params);
const commonT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("common", key, params);

type PorkbunRecord = {
  content: string;
};

type PorkbunResponse = {
  status?: string;
  records?: PorkbunRecord[];
  message?: string;
};

export const porkbunProvider: DDNSProviderDefinition = {
  name: "porkbun",
  label: "Porkbun",
  fields: [
    { key: "api_key", label: "API Key", type: "text", placeholder: "Porkbun API Key", required: true },
    { key: "secret_api_key", label: "Secret API Key", type: "password", placeholder: "Porkbun Secret API Key", required: true },
    { key: "root_domain", label: commonT("fields.root_domain.label"), type: "text", placeholder: "example.com", required: true },
    { key: "domain", label: commonT("fields.domain.label"), type: "text", placeholder: "home.example.com", required: true },
    { key: "ttl", label: "TTL", type: "text", placeholder: "600", required: false, description: commonT("fields.ttl.description", { seconds: 600 }) },
  ],
};

async function porkbunRequest(
  context: DDNSProviderContext,
  path: string,
  body: Record<string, unknown>,
): Promise<PorkbunResponse> {
  const { config, http } = context;
  const response = await http.fetch(`${PORKBUN_ENDPOINT}${path}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      apikey: config.api_key,
      secretapikey: config.secret_api_key,
      ...body,
    }),
    signal: AbortSignal.timeout(getTimeoutMs()),
  });

  return parseJsonResponse<PorkbunResponse>(response);
}

export async function porkbunUpdate(
  context: DDNSProviderContext,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const { config } = context;
  const { api_key, secret_api_key, root_domain, domain } = config;
  if (!api_key || !secret_api_key || !root_domain || !domain) {
    return { success: false, message: porkbunT("configIncomplete") };
  }

  const ttl = String(toPositiveInt(config.ttl, 600));
  const parsed = splitDomain(domain, root_domain);

  return updateDualStack("Porkbun", ipv4, ipv6, async (recordType, ip) => {
    const list = await porkbunRequest(
      context,
      `/retrieveByNameType/${encodeURIComponent(parsed.rootDomain)}/${encodeURIComponent(recordType)}/${encodeURIComponent(parsed.recordName)}`,
      {},
    );

    if (list.status !== "SUCCESS") {
      throw new Error(list.message || porkbunT("queryRecordFailed"));
    }

    const record = list.records?.[0];
    if (record) {
      if (record.content === ip) {
        return;
      }

      const result = await porkbunRequest(
        context,
        `/editByNameType/${encodeURIComponent(parsed.rootDomain)}/${encodeURIComponent(recordType)}/${encodeURIComponent(parsed.recordName)}`,
        {
          content: ip,
          ttl,
        },
      );

      if (result.status !== "SUCCESS") {
        throw new Error(result.message || porkbunT("updateRecordFailed"));
      }
      return;
    }

    const result = await porkbunRequest(context, `/create/${encodeURIComponent(parsed.rootDomain)}`, {
      name: parsed.recordName,
      type: recordType,
      content: ip,
      ttl,
    });

    if (result.status !== "SUCCESS") {
      throw new Error(result.message || porkbunT("createRecordFailed"));
    }
  });
}
