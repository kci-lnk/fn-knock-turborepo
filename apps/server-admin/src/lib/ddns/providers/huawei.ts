import type { DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import {
  applyHuaweiSdkAuth,
  getTimeoutMs,
  parseJsonResponse,
  splitDomain,
  toPositiveInt,
  updateDualStack,
} from "./helpers";

const HUAWEI_DNS_ENDPOINT = "https://dns.myhuaweicloud.com";

type HuaweiZoneResponse = {
  zones?: Array<{
    id: string;
    name: string;
  }>;
};

type HuaweiRecordset = {
  id: string;
  zone_id: string;
  name: string;
  type: string;
  ttl: number;
  records: string[];
};

type HuaweiRecordsetListResponse = {
  recordsets?: HuaweiRecordset[];
};

export const huaweiProvider: DDNSProviderDefinition = {
  name: "huaweicloud",
  label: "华为云 DNS",
  fields: [
    { key: "access_key_id", label: "Access Key", type: "text", placeholder: "华为云 AK", required: true },
    { key: "secret_access_key", label: "Secret Key", type: "password", placeholder: "华为云 SK", required: true },
    { key: "root_domain", label: "根域名", type: "text", placeholder: "example.com", required: true },
    { key: "domain", label: "完整域名", type: "text", placeholder: "home.example.com", required: true },
    { key: "ttl", label: "TTL", type: "text", placeholder: "300", required: false, description: "默认 300 秒" },
  ],
};

async function huaweiRequest<T>(
  config: Record<string, string>,
  path: string,
  method: "GET" | "POST" | "PUT",
  body?: Record<string, unknown>,
): Promise<T> {
  const accessKeyId = config.access_key_id;
  const secretAccessKey = config.secret_access_key;
  if (!accessKeyId || !secretAccessKey) {
    throw new Error("华为云 DNS 配置不完整");
  }

  const payload = body ? JSON.stringify(body) : "";
  const request = new Request(`${HUAWEI_DNS_ENDPOINT}${path}`, {
    method,
    headers: {
      "Content-Type": "application/json",
    },
    body: payload || undefined,
    signal: AbortSignal.timeout(getTimeoutMs()),
  });

  applyHuaweiSdkAuth(request, accessKeyId, secretAccessKey, payload);
  const response = await fetch(request);
  return parseJsonResponse<T>(response);
}

export async function huaweiUpdate(
  config: Record<string, string>,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const { access_key_id, secret_access_key, root_domain, domain } = config;
  if (!access_key_id || !secret_access_key || !root_domain || !domain) {
    return { success: false, message: "华为云 DNS 配置不完整" };
  }

  const ttl = toPositiveInt(config.ttl, 300);
  const parsed = splitDomain(domain, root_domain);
  const fqdnWithDot = `${parsed.fqdn}.`;

  const zoneResponse = await huaweiRequest<HuaweiZoneResponse>(
    config,
    `/v2/zones?name=${encodeURIComponent(parsed.rootDomain)}`,
    "GET",
  );
  const zone =
    zoneResponse.zones?.find((item) => item.name === `${parsed.rootDomain}.`) ||
    zoneResponse.zones?.[0];

  if (!zone) {
    return { success: false, message: `未找到华为云 Zone: ${parsed.rootDomain}` };
  }

  return updateDualStack("华为云 DNS", ipv4, ipv6, async (recordType, ip) => {
    const records = await huaweiRequest<HuaweiRecordsetListResponse>(
      config,
      `/v2/recordsets?type=${encodeURIComponent(recordType)}&name=${encodeURIComponent(fqdnWithDot)}`,
      "GET",
    );

    const existing = (records.recordsets || []).find((record) => record.name === fqdnWithDot);
    if (existing) {
      if (existing.records[0] === ip) {
        return;
      }

      await huaweiRequest<HuaweiRecordset>(
        config,
        `/v2/zones/${encodeURIComponent(existing.zone_id)}/recordsets/${encodeURIComponent(existing.id)}`,
        "PUT",
        {
          records: [ip],
          ttl,
        },
      );
      return;
    }

    await huaweiRequest<HuaweiRecordset>(
      config,
      `/v2/zones/${encodeURIComponent(zone.id)}/recordsets`,
      "POST",
      {
        name: fqdnWithDot,
        type: recordType,
        ttl,
        records: [ip],
      },
    );
  });
}
