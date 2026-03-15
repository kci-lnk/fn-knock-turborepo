import type {
  DDNSProviderContext,
  DDNSProviderDefinition,
  DDNSUpdateResult,
} from "../types";
import {
  requestTencentCloudJson,
  splitDomain,
  toPositiveInt,
  updateDualStack,
} from "./helpers";

const TENCENTCLOUD_DNSPOD_HOST = "dnspod.tencentcloudapi.com";
const TENCENTCLOUD_DNSPOD_SERVICE = "dnspod";
const TENCENTCLOUD_DNSPOD_VERSION = "2021-03-23";

type TencentCloudDescribeRecordListResponse = {
  RecordList?: Array<{
    Name?: string;
    RecordId: number;
    Type?: string;
    Value?: string;
    Line?: string;
    LineId?: string;
    TTL?: number;
  }>;
};

type TencentCloudRecordChangeResponse = {
  RecordId?: number;
};

export const tencentcloudProvider: DDNSProviderDefinition = {
  name: "tencentcloud",
  label: "腾讯云 DNS",
  fields: [
    {
      key: "secret_id",
      label: "SecretId",
      type: "text",
      placeholder: "AKID...",
      required: true,
    },
    {
      key: "secret_key",
      label: "SecretKey",
      type: "password",
      placeholder: "腾讯云 SecretKey",
      required: true,
    },
    {
      key: "root_domain",
      label: "根域名",
      type: "text",
      placeholder: "example.com",
      required: true,
      description: "用于确定 Zone，例如 example.com",
    },
    {
      key: "domain",
      label: "完整域名",
      type: "text",
      placeholder: "home.example.com",
      required: true,
      description: "要更新的完整主机名",
    },
    {
      key: "record_line",
      label: "线路",
      type: "text",
      placeholder: "默认",
      required: false,
      description: "默认使用“默认”线路",
    },
    {
      key: "record_line_id",
      label: "线路 ID",
      type: "text",
      placeholder: "0",
      required: false,
      description: "可选；如填写将优先使用线路 ID",
    },
    {
      key: "ttl",
      label: "TTL",
      type: "text",
      placeholder: "600",
      required: false,
      description: "默认 600 秒",
    },
  ],
};

async function tencentcloudRequest<T>(
  context: DDNSProviderContext,
  action: string,
  payload: Record<string, unknown>,
): Promise<T> {
  const { config, http } = context;
  const secretId = config.secret_id?.trim();
  const secretKey = config.secret_key?.trim();
  if (!secretId || !secretKey) {
    throw new Error("腾讯云 DNS 配置不完整");
  }

  return requestTencentCloudJson<T>(http, {
    action,
    host: TENCENTCLOUD_DNSPOD_HOST,
    payload,
    secretId,
    secretKey,
    service: TENCENTCLOUD_DNSPOD_SERVICE,
    version: TENCENTCLOUD_DNSPOD_VERSION,
  });
}

async function describeTencentCloudRecordList(
  context: DDNSProviderContext,
  payload: Record<string, unknown>,
): Promise<TencentCloudDescribeRecordListResponse> {
  try {
    return await tencentcloudRequest<TencentCloudDescribeRecordListResponse>(
      context,
      "DescribeRecordList",
      payload,
    );
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (message.startsWith("ResourceNotFound.NoDataOfRecord:")) {
      return { RecordList: [] };
    }
    throw error;
  }
}

export async function tencentcloudUpdate(
  context: DDNSProviderContext,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const { config } = context;
  const secretId = config.secret_id?.trim();
  const secretKey = config.secret_key?.trim();
  const rootDomain = config.root_domain?.trim();
  const domain = config.domain?.trim();
  if (!secretId || !secretKey || !rootDomain || !domain) {
    return { success: false, message: "腾讯云 DNS 配置不完整" };
  }

  const ttl = toPositiveInt(config.ttl, 600);
  const parsed = splitDomain(domain, rootDomain);
  const recordLine = config.record_line?.trim() || "默认";
  const recordLineId = config.record_line_id?.trim();

  return updateDualStack("腾讯云 DNS", ipv4, ipv6, async (recordType, ip) => {
    const basePayload: Record<string, unknown> = {
      Domain: parsed.rootDomain,
      RecordType: recordType,
    };

    if (recordLineId) {
      basePayload.RecordLineId = recordLineId;
    } else {
      basePayload.RecordLine = recordLine;
    }

    const list = await describeTencentCloudRecordList(context, {
      ...basePayload,
      Limit: 100,
      Offset: 0,
      Subdomain: parsed.recordName,
    });

    const existing = (list.RecordList || []).find((record) => {
      if ((record.Name || parsed.recordName) !== parsed.recordName) {
        return false;
      }
      if ((record.Type || recordType) !== recordType) {
        return false;
      }
      if (recordLineId) {
        return (record.LineId || "") === recordLineId;
      }
      return (record.Line || "默认") === recordLine;
    });

    if (existing) {
      if (existing.Value === ip) {
        return;
      }

      const result =
        await tencentcloudRequest<TencentCloudRecordChangeResponse>(
          context,
          "ModifyRecord",
          {
            ...basePayload,
            RecordId: existing.RecordId,
            SubDomain: parsed.recordName,
            TTL: ttl,
            Value: ip,
          },
        );

      if (!result.RecordId) {
        throw new Error("腾讯云未返回更新后的 RecordId");
      }
      return;
    }

    const result = await tencentcloudRequest<TencentCloudRecordChangeResponse>(
      context,
      "CreateRecord",
      {
        ...basePayload,
        SubDomain: parsed.recordName,
        TTL: ttl,
        Value: ip,
      },
    );

    if (!result.RecordId) {
      throw new Error("腾讯云未返回创建后的 RecordId");
    }
  });
}
