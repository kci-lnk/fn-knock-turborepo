import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import {
  ddnsProviderT,
  getTimeoutMs,
  parseJsonResponse,
  splitDomain,
  toPositiveInt,
  updateDualStack,
} from "./helpers";

const DNSPOD_RECORD_LIST_API = "https://dnsapi.cn/Record.List";
const DNSPOD_RECORD_MODIFY_API = "https://dnsapi.cn/Record.Modify";
const DNSPOD_RECORD_CREATE_API = "https://dnsapi.cn/Record.Create";
const DNSPOD_DEFAULT_LINE = "\u9ed8\u8ba4";
const dnspodT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("dnspod", key, params);
const commonT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("common", key, params);

type DnspodResponse = {
  status?: {
    code?: string;
    message?: string;
  };
  records?: Array<{
    id: string;
    value: string;
  }>;
};

export const dnspodProvider: DDNSProviderDefinition = {
  name: "dnspod",
  label: "DNSPod",
  fields: [
    { key: "token_id", label: "Token ID", type: "text", placeholder: "DNSPod Token ID", required: true },
    { key: "token_key", label: "Token Key", type: "password", placeholder: "DNSPod Token Key", required: true },
    { key: "root_domain", label: commonT("fields.root_domain.label"), type: "text", placeholder: "example.com", required: true },
    { key: "domain", label: commonT("fields.domain.label"), type: "text", placeholder: "home.example.com", required: true },
    { key: "record_line", label: dnspodT("fields.record_line.label"), type: "text", placeholder: dnspodT("defaultLine"), required: false, description: dnspodT("fields.record_line.description") },
    { key: "ttl", label: "TTL", type: "text", placeholder: "600", required: false, description: commonT("fields.ttl.description", { seconds: 600 }) },
  ],
};

async function dnspodRequest(
  api: string,
  context: DDNSProviderContext,
  params: Record<string, string>,
): Promise<DnspodResponse> {
  const { config, http } = context;
  const form = new URLSearchParams({
    login_token: `${config.token_id},${config.token_key}`,
    format: "json",
    ...params,
  });

  const response = await http.fetch(api, {
    method: "POST",
    headers: {
      "Content-Type": "application/x-www-form-urlencoded",
    },
    body: form.toString(),
    signal: AbortSignal.timeout(getTimeoutMs()),
  });

  return parseJsonResponse<DnspodResponse>(response);
}

export async function dnspodUpdate(
  context: DDNSProviderContext,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const { config } = context;
  const { token_id, token_key, root_domain, domain } = config;
  if (!token_id || !token_key || !root_domain || !domain) {
    return { success: false, message: dnspodT("configIncomplete") };
  }

  const ttl = String(toPositiveInt(config.ttl, 600));
  const parsed = splitDomain(domain, root_domain);
  const recordLine = config.record_line || DNSPOD_DEFAULT_LINE;

  return updateDualStack("DNSPod", ipv4, ipv6, async (recordType, ip) => {
    const list = await dnspodRequest(DNSPOD_RECORD_LIST_API, context, {
      domain: parsed.rootDomain,
      sub_domain: parsed.recordName,
      record_type: recordType,
      record_line: recordLine,
    });

    if (list.status?.code !== "1") {
      throw new Error(list.status?.message || dnspodT("queryRecordFailed"));
    }

    const record = list.records?.[0];
    if (record) {
      if (record.value === ip) {
        return;
      }

      const result = await dnspodRequest(DNSPOD_RECORD_MODIFY_API, context, {
        domain: parsed.rootDomain,
        sub_domain: parsed.recordName,
        record_type: recordType,
        record_line: recordLine,
        record_id: record.id,
        value: ip,
        ttl,
      });

      if (result.status?.code !== "1") {
        throw new Error(result.status?.message || dnspodT("updateRecordFailed"));
      }
      return;
    }

    const result = await dnspodRequest(DNSPOD_RECORD_CREATE_API, context, {
      domain: parsed.rootDomain,
      sub_domain: parsed.recordName,
      record_type: recordType,
      record_line: recordLine,
      value: ip,
      ttl,
    });

    if (result.status?.code !== "1") {
      throw new Error(result.status?.message || dnspodT("createRecordFailed"));
    }
  });
}
