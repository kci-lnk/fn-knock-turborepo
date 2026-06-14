import type {
  DDNSProviderContext,
  DDNSProviderDefinition,
  DDNSUpdateResult,
} from "../types";
import {
  ddnsProviderT,
  requestTencentCloudJson,
  splitDomain,
  toPositiveInt,
  updateDualStack,
} from "./helpers";

const TENCENTCLOUD_DNSPOD_HOST = "dnspod.tencentcloudapi.com";
const TENCENTCLOUD_DNSPOD_SERVICE = "dnspod";
const TENCENTCLOUD_DNSPOD_VERSION = "2021-03-23";
const TENCENTCLOUD_DEFAULT_LINE = "\u9ed8\u8ba4";
const tencentcloudT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("tencentcloud", key, params);
const commonT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("common", key, params);

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
  label: tencentcloudT("label"),
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
      placeholder: tencentcloudT("fields.secret_key.placeholder"),
      required: true,
    },
    {
      key: "root_domain",
      label: commonT("fields.root_domain.label"),
      type: "text",
      placeholder: "example.com",
      required: true,
      description: commonT("fields.root_domain.description"),
    },
    {
      key: "domain",
      label: commonT("fields.domain.label"),
      type: "text",
      placeholder: "home.example.com",
      required: true,
      description: commonT("fields.domain.hostDescription"),
    },
    {
      key: "record_line",
      label: tencentcloudT("fields.record_line.label"),
      type: "text",
      placeholder: tencentcloudT("defaultLine"),
      required: false,
      description: tencentcloudT("fields.record_line.description"),
    },
    {
      key: "record_line_id",
      label: tencentcloudT("fields.record_line_id.label"),
      type: "text",
      placeholder: "0",
      required: false,
      description: tencentcloudT("fields.record_line_id.description"),
    },
    {
      key: "ttl",
      label: "TTL",
      type: "text",
      placeholder: "600",
      required: false,
      description: commonT("fields.ttl.description", { seconds: 600 }),
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
    throw new Error(tencentcloudT("configIncomplete"));
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
    return { success: false, message: tencentcloudT("configIncomplete") };
  }

  const ttl = toPositiveInt(config.ttl, 600);
  const parsed = splitDomain(domain, rootDomain);
  const recordLine = config.record_line?.trim() || TENCENTCLOUD_DEFAULT_LINE;
  const recordLineId = config.record_line_id?.trim();

  return updateDualStack(tencentcloudT("label"), ipv4, ipv6, async (recordType, ip) => {
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
      return (record.Line || TENCENTCLOUD_DEFAULT_LINE) === recordLine;
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
        throw new Error(tencentcloudT("missingUpdatedRecordId"));
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
      throw new Error(tencentcloudT("missingCreatedRecordId"));
    }
  });
}
