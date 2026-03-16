import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import {
  normalizeDomain,
  requestAliyunAcs3Json,
  toPositiveInt,
} from "./helpers";

const ESA_ENDPOINT = "https://esa.cn-hangzhou.aliyuncs.com/";
const ESA_API_VERSION = "2024-09-10";

type EsaSite = {
  SiteId?: number;
  SiteName?: string;
};

type EsaRecord = {
  BizName?: string;
  Data?: {
    Value?: string;
  };
  Proxied?: boolean;
  RecordId?: number;
  RecordName?: string;
  RecordType?: string;
  Ttl?: number;
};

type EsaListSitesResponse = {
  RequestId?: string;
  Sites?: EsaSite[];
  TotalCount?: number;
  Code?: string;
  Message?: string;
};

type EsaListRecordsResponse = {
  Records?: EsaRecord[];
  RequestId?: string;
  TotalCount?: number;
  Code?: string;
  Message?: string;
};

type EsaCreateRecordResponse = {
  RecordId?: number;
  RequestId?: string;
  Code?: string;
  Message?: string;
};

type EsaUpdateRecordResponse = {
  RequestId?: string;
  Code?: string;
  Message?: string;
};

export const esaProvider: DDNSProviderDefinition = {
  name: "esa",
  label: "阿里云 ESA DNS",
  fields: [
    { key: "access_key_id", label: "AccessKey ID", type: "text", placeholder: "LTAI...", required: true },
    { key: "access_key_secret", label: "AccessKey Secret", type: "password", placeholder: "阿里云 AccessKey Secret", required: true },
    { key: "site_name", label: "站点名称", type: "text", placeholder: "example.com", required: true, description: "ESA 站点名称，通常就是根域名；如已填写 Site ID，此项仅作兜底查询" },
    { key: "site_id", label: "Site ID", type: "text", placeholder: "123456", required: false, description: "可选，填写后将直接操作该站点，避免每次先查询站点列表" },
    { key: "domain", label: "完整域名", type: "text", placeholder: "home.example.com", required: true, description: "要更新的完整主机名" },
    {
      key: "proxied",
      label: "ESA 代理",
      type: "select",
      required: false,
      options: [
        { label: "仅解析", value: "false" },
        { label: "开启代理", value: "true" },
      ],
      description: "默认仅解析；如开启代理，将自动附带业务类型",
    },
    {
      key: "biz_name",
      label: "业务类型",
      type: "select",
      required: false,
      options: [
        { label: "网页", value: "web" },
        { label: "接口", value: "api" },
        { label: "音视频", value: "image_video" },
      ],
      description: "仅在开启 ESA 代理时生效，默认 web",
    },
    { key: "ttl", label: "TTL", type: "text", placeholder: "30", required: false, description: "默认 30 秒" },
  ],
};

async function esaRequest<T extends { Code?: string; Message?: string }>(
  context: DDNSProviderContext,
  action: string,
  method: "GET" | "POST",
  options: {
    query?: Record<string, unknown>;
    formData?: Record<string, unknown>;
  } = {},
): Promise<T> {
  const { config, http } = context;
  const accessKeyId = config.access_key_id?.trim();
  const accessKeySecret = config.access_key_secret?.trim();
  if (!accessKeyId || !accessKeySecret) {
    throw new Error("阿里云 ESA DNS 配置不完整");
  }

  return requestAliyunAcs3Json<T>(http, {
    accessKeyId,
    accessKeySecret,
    action,
    endpoint: ESA_ENDPOINT,
    formData: options.formData,
    method,
    query: options.query,
    version: ESA_API_VERSION,
  });
}

async function resolveSiteId(context: DDNSProviderContext): Promise<string> {
  const { config } = context;
  const siteId = config.site_id?.trim();
  if (siteId) {
    return siteId;
  }

  const siteName = normalizeDomain(config.site_name || "");
  if (!siteName) {
    throw new Error("阿里云 ESA DNS 缺少站点名称");
  }

  const result = await esaRequest<EsaListSitesResponse>(context, "ListSites", "GET", {
    query: {
      PageNumber: 1,
      PageSize: 100,
      SiteName: siteName,
      SiteSearchType: "exact",
    },
  });

  const matched = (result.Sites || []).find((site) => normalizeDomain(site.SiteName || "") === siteName);
  if (!matched?.SiteId) {
    throw new Error(`未找到 ESA 站点: ${siteName}`);
  }

  return String(matched.SiteId);
}

function buildRecordPayload(
  value: string,
  ttl: number,
  proxied: boolean,
  bizName?: string,
): Record<string, unknown> {
  return {
    BizName: proxied ? (bizName || "web") : undefined,
    Data: JSON.stringify({ Value: value }),
    Proxied: proxied,
    Ttl: ttl,
    Type: "A/AAAA",
  };
}

function normalizeRecordValues(value: string | undefined): string[] {
  return (value || "")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean)
    .sort();
}

function isSameRecordValues(left: string | undefined, right: string): boolean {
  const leftValues = normalizeRecordValues(left);
  const rightValues = normalizeRecordValues(right);
  if (leftValues.length !== rightValues.length) {
    return false;
  }

  return leftValues.every((value, index) => value === rightValues[index]);
}

export async function esaUpdate(
  context: DDNSProviderContext,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const { config } = context;
  const domain = normalizeDomain(config.domain || "");
  const siteName = normalizeDomain(config.site_name || "");
  const accessKeyId = config.access_key_id?.trim();
  const accessKeySecret = config.access_key_secret?.trim();
  if (!accessKeyId || !accessKeySecret || !domain || (!siteName && !config.site_id?.trim())) {
    return { success: false, message: "阿里云 ESA DNS 配置不完整" };
  }

  const ttl = toPositiveInt(config.ttl, 30);
  const proxied = config.proxied === "true";
  const bizName = proxied ? (config.biz_name?.trim() || "web") : undefined;
  const siteId = await resolveSiteId(context);
  const recordValue = [ipv4, ipv6]
    .filter((item): item is string => Boolean(item))
    .join(",");

  if (!recordValue) {
    return { success: false, message: "阿里云 ESA DNS 缺少可更新的 IP 地址" };
  }

  const records = await esaRequest<EsaListRecordsResponse>(context, "ListRecords", "GET", {
    query: {
      PageNumber: 1,
      PageSize: 100,
      RecordMatchType: "exact",
      RecordName: domain,
      SiteId: siteId,
      Type: "A/AAAA",
    },
  });

  const existingRecords = (records.Records || []).filter((record) => {
    return normalizeDomain(record.RecordName || "") === domain
      && (record.RecordType || "").toUpperCase() === "A/AAAA";
  });

  if (existingRecords.length === 0) {
    const result = await esaRequest<EsaCreateRecordResponse>(context, "CreateRecord", "POST", {
      query: {
        RecordName: domain,
        SiteId: siteId,
        ...buildRecordPayload(recordValue, ttl, proxied, bizName),
      },
    });

    if (!result.RecordId) {
      throw new Error("CreateFailed: 创建记录失败");
    }

    return {
      success: true,
      message: "阿里云 ESA DNS 更新成功",
      ipv4Updated: Boolean(ipv4),
      ipv6Updated: Boolean(ipv6),
    };
  }

  for (const record of existingRecords) {
    const currentValue = record.Data?.Value || "";
    const currentTtl = record.Ttl ?? ttl;
    const currentProxied = record.Proxied ?? false;
    const currentBizName = record.BizName || "";
    const desiredBizName = bizName || "";

    if (
      isSameRecordValues(currentValue, recordValue)
      && currentTtl === ttl
      && currentProxied === proxied
      && currentBizName === desiredBizName
    ) {
      continue;
    }

    if (!record.RecordId) {
      throw new Error("UpdateFailed: 记录缺少 RecordId");
    }

    await esaRequest<EsaUpdateRecordResponse>(context, "UpdateRecord", "POST", {
      query: {
        RecordId: record.RecordId,
        ...buildRecordPayload(recordValue, ttl, proxied, bizName),
      },
    });
  }

  return {
    success: true,
    message: "阿里云 ESA DNS 更新成功",
    ipv4Updated: Boolean(ipv4),
    ipv6Updated: Boolean(ipv6),
  };
}
