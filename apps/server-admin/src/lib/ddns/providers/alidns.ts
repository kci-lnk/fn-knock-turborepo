import type { DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import {
  buildAliyunSignedParams,
  getTimeoutMs,
  parseJsonResponse,
  splitDomain,
  toPositiveInt,
  updateDualStack,
} from "./helpers";

const ALIDNS_ENDPOINT = "https://alidns.aliyuncs.com/";

type AlidnsRecord = {
  RecordId: string;
  Value: string;
};

type AlidnsDescribeResponse = {
  TotalCount?: number;
  DomainRecords?: {
    Record?: AlidnsRecord[];
  };
  Code?: string;
  Message?: string;
};

type AlidnsChangeResponse = {
  RecordId?: string;
  Code?: string;
  Message?: string;
};

export const alidnsProvider: DDNSProviderDefinition = {
  name: "alidns",
  label: "阿里云 DNS",
  fields: [
    { key: "access_key_id", label: "AccessKey ID", type: "text", placeholder: "LTAI...", required: true },
    { key: "access_key_secret", label: "AccessKey Secret", type: "password", placeholder: "阿里云 AccessKey Secret", required: true },
    { key: "root_domain", label: "根域名", type: "text", placeholder: "example.com", required: true, description: "用于确定 Zone，例如 example.com" },
    { key: "domain", label: "完整域名", type: "text", placeholder: "home.example.com", required: true, description: "要更新的完整主机名" },
    { key: "ttl", label: "TTL", type: "text", placeholder: "600", required: false, description: "默认 600 秒" },
  ],
};

async function alidnsRequest<T>(
  config: Record<string, string>,
  params: Record<string, string>,
): Promise<T> {
  const accessKeyId = config.access_key_id;
  const accessKeySecret = config.access_key_secret;
  if (!accessKeyId || !accessKeySecret) {
    throw new Error("阿里云 DNS 配置不完整");
  }

  const query = buildAliyunSignedParams(accessKeyId, accessKeySecret, params);
  const response = await fetch(`${ALIDNS_ENDPOINT}?${query.toString()}`, {
    signal: AbortSignal.timeout(getTimeoutMs()),
  });
  const data = await parseJsonResponse<T>(response);
  return data;
}

export async function alidnsUpdate(
  config: Record<string, string>,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const { access_key_id, access_key_secret, root_domain, domain } = config;
  if (!access_key_id || !access_key_secret || !root_domain || !domain) {
    return { success: false, message: "阿里云 DNS 配置不完整" };
  }

  const ttl = String(toPositiveInt(config.ttl, 600));
  const parsed = splitDomain(domain, root_domain);

  return updateDualStack("阿里云 DNS", ipv4, ipv6, async (recordType, ip) => {
    const records = await alidnsRequest<AlidnsDescribeResponse>(config, {
      Action: "DescribeSubDomainRecords",
      DomainName: parsed.rootDomain,
      SubDomain: parsed.fqdn,
      Type: recordType,
    });

    if (records.Code) {
      throw new Error(`${records.Code}: ${records.Message || "请求失败"}`);
    }

    const existing = records.DomainRecords?.Record?.[0];
    if (existing) {
      if (existing.Value === ip) {
        return;
      }

      const result = await alidnsRequest<AlidnsChangeResponse>(config, {
        Action: "UpdateDomainRecord",
        RR: parsed.recordName,
        RecordId: existing.RecordId,
        Type: recordType,
        Value: ip,
        TTL: ttl,
      });

      if (result.Code || !result.RecordId) {
        throw new Error(`${result.Code || "UpdateFailed"}: ${result.Message || "更新失败"}`);
      }
      return;
    }

    const result = await alidnsRequest<AlidnsChangeResponse>(config, {
      Action: "AddDomainRecord",
      DomainName: parsed.rootDomain,
      RR: parsed.recordName,
      Type: recordType,
      Value: ip,
      TTL: ttl,
    });

    if (result.Code || !result.RecordId) {
      throw new Error(`${result.Code || "CreateFailed"}: ${result.Message || "创建失败"}`);
    }
  });
}
