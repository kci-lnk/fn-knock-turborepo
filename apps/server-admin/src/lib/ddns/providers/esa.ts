import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import {
  ddnsProviderT,
  normalizeDomain,
  requestAliyunAcs3Json,
  toPositiveInt,
} from "./helpers";

const ESA_ENDPOINT = "https://esa.cn-hangzhou.aliyuncs.com/";
const ESA_API_VERSION = "2024-09-10";
const esaT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("esa", key, params);
const commonT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("common", key, params);

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
  label: esaT("label"),
  fields: [
    { key: "access_key_id", label: "AccessKey ID", type: "text", placeholder: "LTAI...", required: true },
    { key: "access_key_secret", label: "AccessKey Secret", type: "password", placeholder: esaT("fields.access_key_secret.placeholder"), required: true },
    { key: "site_name", label: esaT("fields.site_name.label"), type: "text", placeholder: "example.com", required: true, description: esaT("fields.site_name.description") },
    { key: "site_id", label: "Site ID", type: "text", placeholder: "123456", required: false, description: esaT("fields.site_id.description") },
    { key: "domain", label: commonT("fields.domain.label"), type: "text", placeholder: "home.example.com", required: true, description: commonT("fields.domain.hostDescription") },
    {
      key: "proxied",
      label: esaT("fields.proxied.label"),
      type: "select",
      required: false,
      options: [
        { label: esaT("fields.proxied.options.dnsOnly"), value: "false" },
        { label: esaT("fields.proxied.options.enabled"), value: "true" },
      ],
      description: esaT("fields.proxied.description"),
    },
    {
      key: "biz_name",
      label: esaT("fields.biz_name.label"),
      type: "select",
      required: false,
      options: [
        { label: esaT("fields.biz_name.options.web"), value: "web" },
        { label: esaT("fields.biz_name.options.api"), value: "api" },
        { label: esaT("fields.biz_name.options.imageVideo"), value: "image_video" },
      ],
      description: esaT("fields.biz_name.description"),
    },
    { key: "ttl", label: "TTL", type: "text", placeholder: "30", required: false, description: commonT("fields.ttl.description", { seconds: 30 }) },
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
    throw new Error(esaT("configIncomplete"));
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
    throw new Error(esaT("siteNameMissing"));
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
    throw new Error(esaT("siteNotFound", { site: siteName }));
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
    return { success: false, message: esaT("configIncomplete") };
  }

  const ttl = toPositiveInt(config.ttl, 30);
  const proxied = config.proxied === "true";
  const bizName = proxied ? (config.biz_name?.trim() || "web") : undefined;
  const siteId = await resolveSiteId(context);
  const recordValue = [ipv4, ipv6]
    .filter((item): item is string => Boolean(item))
    .join(",");

  if (!recordValue) {
    return { success: false, message: esaT("noIpAvailable") };
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
      throw new Error(esaT("createRecordFailed"));
    }

    return {
      success: true,
      message: esaT("success"),
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
      throw new Error(esaT("recordIdMissing"));
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
    message: esaT("success"),
    ipv4Updated: Boolean(ipv4),
    ipv6Updated: Boolean(ipv6),
  };
}
