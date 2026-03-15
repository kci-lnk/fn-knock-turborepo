import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
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
  Line?: string;
  RR?: string;
  Type?: string;
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
    { key: "line", label: "线路", type: "text", placeholder: "default", required: false, description: "默认使用阿里云“default”线路" },
    { key: "ttl", label: "TTL", type: "text", placeholder: "600", required: false, description: "默认 600 秒" },
  ],
};

async function alidnsRequest<T>(
  context: DDNSProviderContext,
  params: Record<string, string>,
): Promise<T> {
  const { config, http } = context;
  const accessKeyId = config.access_key_id?.trim();
  const accessKeySecret = config.access_key_secret?.trim();
  if (!accessKeyId || !accessKeySecret) {
    throw new Error("阿里云 DNS 配置不完整");
  }

  const body = buildAliyunSignedParams(accessKeyId, accessKeySecret, params, "POST");
  const response = await http.fetch(ALIDNS_ENDPOINT, {
    body,
    headers: {
      "Content-Type": "application/x-www-form-urlencoded",
    },
    method: "POST",
    signal: AbortSignal.timeout(getTimeoutMs()),
  });
  return parseJsonResponse<T>(response);
}

export async function alidnsUpdate(
  context: DDNSProviderContext,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const { config } = context;
  const accessKeyId = config.access_key_id?.trim();
  const accessKeySecret = config.access_key_secret?.trim();
  const rootDomain = config.root_domain?.trim();
  const domain = config.domain?.trim();
  if (!accessKeyId || !accessKeySecret || !rootDomain || !domain) {
    return { success: false, message: "阿里云 DNS 配置不完整" };
  }

  const ttl = String(toPositiveInt(config.ttl, 600));
  const parsed = splitDomain(domain, rootDomain);
  const line = (config.line || "default").trim() || "default";

  return updateDualStack("阿里云 DNS", ipv4, ipv6, async (recordType, ip) => {
    const records = await alidnsRequest<AlidnsDescribeResponse>(context, {
      Action: "DescribeSubDomainRecords",
      DomainName: parsed.rootDomain,
      Line: line,
      PageSize: "100",
      SubDomain: parsed.fqdn,
      Type: recordType,
    });

    if (records.Code) {
      throw new Error(`${records.Code}: ${records.Message || "请求失败"}`);
    }

    const existingRecords = (records.DomainRecords?.Record || []).filter((record) => {
      return (record.RR || parsed.recordName) === parsed.recordName
        && (record.Type || recordType) === recordType
        && (record.Line || "default") === line;
    });

    if (existingRecords.length > 0) {
      for (const record of existingRecords) {
        if (record.Value === ip) {
          continue;
        }

        const result = await alidnsRequest<AlidnsChangeResponse>(context, {
          Action: "UpdateDomainRecord",
          Line: line,
          RR: parsed.recordName,
          RecordId: record.RecordId,
          TTL: ttl,
          Type: recordType,
          Value: ip,
        });

        if (result.Code || !result.RecordId) {
          throw new Error(`${result.Code || "UpdateFailed"}: ${result.Message || "更新失败"}`);
        }
      }
      return;
    }

    const result = await alidnsRequest<AlidnsChangeResponse>(context, {
      Action: "AddDomainRecord",
      DomainName: parsed.rootDomain,
      Line: line,
      RR: parsed.recordName,
      TTL: ttl,
      Type: recordType,
      Value: ip,
    });

    if (result.Code || !result.RecordId) {
      throw new Error(`${result.Code || "CreateFailed"}: ${result.Message || "创建失败"}`);
    }
  });
}
