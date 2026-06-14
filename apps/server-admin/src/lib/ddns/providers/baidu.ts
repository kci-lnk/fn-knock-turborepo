import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import {
  applyBaiduBceAuth,
  ddnsProviderT,
  getTimeoutMs,
  parseJsonResponse,
  splitDomain,
  toPositiveInt,
  updateDualStack,
} from "./helpers";

const BAIDU_ENDPOINT = "https://bcd.baidubce.com";
const baiduT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("baidu", key, params);
const commonT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("common", key, params);

type BaiduRecord = {
  recordId: number;
  domain: string;
  view: string;
  ttl: number;
  rdata: string;
  zoneName: string;
};

type BaiduRecordListResponse = {
  totalCount?: number;
  result?: BaiduRecord[];
  code?: string;
  message?: string;
};

export const baiduProvider: DDNSProviderDefinition = {
  name: "baiducloud",
  label: baiduT("label"),
  fields: [
    { key: "access_key_id", label: "Access Key", type: "text", placeholder: baiduT("fields.access_key_id.placeholder"), required: true },
    { key: "secret_access_key", label: "Secret Key", type: "password", placeholder: baiduT("fields.secret_access_key.placeholder"), required: true },
    { key: "root_domain", label: commonT("fields.root_domain.label"), type: "text", placeholder: "example.com", required: true },
    { key: "domain", label: commonT("fields.domain.label"), type: "text", placeholder: "home.example.com", required: true },
    { key: "ttl", label: "TTL", type: "text", placeholder: "300", required: false, description: commonT("fields.ttl.description", { seconds: 300 }) },
  ],
};

async function baiduRequest<T>(
  context: DDNSProviderContext,
  path: string,
  body: Record<string, unknown>,
): Promise<T> {
  const { config, http } = context;
  const accessKeyId = config.access_key_id;
  const secretAccessKey = config.secret_access_key;
  if (!accessKeyId || !secretAccessKey) {
    throw new Error(baiduT("configIncomplete"));
  }

  const request = new Request(`${BAIDU_ENDPOINT}${path}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(getTimeoutMs()),
  });

  applyBaiduBceAuth(request, accessKeyId, secretAccessKey);
  const response = await http.fetch(request);
  const data = await parseJsonResponse<T>(response);
  return data;
}

export async function baiduUpdate(
  context: DDNSProviderContext,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const { config } = context;
  const { access_key_id, secret_access_key, root_domain, domain } = config;
  if (!access_key_id || !secret_access_key || !root_domain || !domain) {
    return { success: false, message: baiduT("configIncomplete") };
  }

  const ttl = toPositiveInt(config.ttl, 300);
  const parsed = splitDomain(domain, root_domain);

  return updateDualStack(baiduT("label"), ipv4, ipv6, async (recordType, ip) => {
    const list = await baiduRequest<BaiduRecordListResponse>(context, "/v1/domain/resolve/list", {
      domain: parsed.rootDomain,
      pageNum: 1,
      pageSize: 1000,
    });

    if (list.code) {
      throw new Error(`${list.code}: ${list.message || baiduT("queryFailed")}`);
    }

    const existing = (list.result || []).find((record) => record.domain === parsed.recordName);
    if (existing) {
      if (existing.rdata === ip) {
        return;
      }

      const result = await baiduRequest<BaiduRecordListResponse>(context, "/v1/domain/resolve/edit", {
        recordId: existing.recordId,
        domain: existing.domain,
        view: existing.view,
        rdType: recordType,
        ttl: existing.ttl || ttl,
        rdata: ip,
        zoneName: existing.zoneName,
      });

      if (result.code) {
        throw new Error(`${result.code}: ${result.message || baiduT("updateFailed")}`);
      }
      return;
    }

    const result = await baiduRequest<BaiduRecordListResponse>(context, "/v1/domain/resolve/add", {
      domain: parsed.recordName,
      rdType: recordType,
      ttl,
      rdata: ip,
      zoneName: parsed.rootDomain,
    });

    if (result.code) {
      throw new Error(`${result.code}: ${result.message || baiduT("createFailed")}`);
    }
  });
}
