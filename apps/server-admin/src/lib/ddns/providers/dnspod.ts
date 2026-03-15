import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import {
  getTimeoutMs,
  parseJsonResponse,
  splitDomain,
  toPositiveInt,
  updateDualStack,
} from "./helpers";

const DNSPOD_RECORD_LIST_API = "https://dnsapi.cn/Record.List";
const DNSPOD_RECORD_MODIFY_API = "https://dnsapi.cn/Record.Modify";
const DNSPOD_RECORD_CREATE_API = "https://dnsapi.cn/Record.Create";

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
    { key: "root_domain", label: "根域名", type: "text", placeholder: "example.com", required: true },
    { key: "domain", label: "完整域名", type: "text", placeholder: "home.example.com", required: true },
    { key: "record_line", label: "线路", type: "text", placeholder: "默认", required: false, description: "默认使用“默认”线路" },
    { key: "ttl", label: "TTL", type: "text", placeholder: "600", required: false, description: "默认 600 秒" },
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
    return { success: false, message: "DNSPod 配置不完整" };
  }

  const ttl = String(toPositiveInt(config.ttl, 600));
  const parsed = splitDomain(domain, root_domain);
  const recordLine = config.record_line || "默认";

  return updateDualStack("DNSPod", ipv4, ipv6, async (recordType, ip) => {
    const list = await dnspodRequest(DNSPOD_RECORD_LIST_API, context, {
      domain: parsed.rootDomain,
      sub_domain: parsed.recordName,
      record_type: recordType,
      record_line: recordLine,
    });

    if (list.status?.code !== "1") {
      throw new Error(list.status?.message || "查询记录失败");
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
        throw new Error(result.status?.message || "更新记录失败");
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
      throw new Error(result.status?.message || "创建记录失败");
    }
  });
}
