import type {
  DDNSProviderContext,
  DDNSProviderDefinition,
  DDNSUpdateResult,
} from "../types";
import { ddnsProviderT, normalizeDomain } from "./helpers";
import {
  EDGEONE_OVERSEAS_ACCESS_MODE_FIELD,
  requestEdgeOneJson,
} from "./edgeone-shared";

const edgeoneCnameT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("edgeone_cname", key, params);

type EdgeOneOriginDetail = {
  HostHeader?: string | null;
  Origin?: string;
  OriginType?: string;
};

type EdgeOneAccelerationDomain = {
  DomainName?: string;
  OriginDetail?: EdgeOneOriginDetail | null;
};

type EdgeOneDescribeAccelerationDomainsResponse = {
  AccelerationDomains?: EdgeOneAccelerationDomain[];
  TotalCount?: number;
};

export const edgeoneCnameProvider: DDNSProviderDefinition = {
  name: "edgeone_cname",
  label: edgeoneCnameT("label"),
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
      placeholder: edgeoneCnameT("fields.secret_key.placeholder"),
      required: true,
    },
    {
      key: "zone_id",
      label: "Zone ID",
      type: "text",
      placeholder: "zone-xxxxxxxx",
      required: true,
      description: edgeoneCnameT("fields.zone_id.description"),
    },
    {
      key: "domain",
      label: edgeoneCnameT("fields.domain.label"),
      type: "text",
      placeholder: "home.example.com",
      required: true,
      description: edgeoneCnameT("fields.domain.description"),
    },
    {
      key: EDGEONE_OVERSEAS_ACCESS_MODE_FIELD,
      label: edgeoneCnameT("fields.overseas_access.label"),
      type: "select",
      required: false,
      options: [
        {
          label: edgeoneCnameT("fields.overseas_access.options.off"),
          value: "off",
        },
        {
          label: edgeoneCnameT("fields.overseas_access.options.blockOverseas"),
          value: "block_overseas",
        },
      ],
      description: edgeoneCnameT("fields.overseas_access.description"),
    },
    {
      key: "endpoint",
      label: "API Endpoint",
      type: "text",
      placeholder: "https://teo.tencentcloudapi.com",
      required: false,
      description: edgeoneCnameT("fields.endpoint.description"),
    },
    {
      key: "region",
      label: "Region",
      type: "text",
      placeholder: edgeoneCnameT("fields.region.placeholder"),
      required: false,
      description: edgeoneCnameT("fields.region.description"),
    },
  ],
};

async function edgeOneCnameRequest<T>(
  context: DDNSProviderContext,
  action: string,
  payload: Record<string, unknown>,
): Promise<T> {
  const { config } = context;
  const secretId = config.secret_id?.trim();
  const secretKey = config.secret_key?.trim();
  if (!secretId || !secretKey) {
    throw new Error(edgeoneCnameT("configIncomplete"));
  }

  return requestEdgeOneJson<T>(context, action, payload);
}

function resolveDesiredOrigin(
  ipv4: string | null,
  ipv6: string | null,
): { family: "ipv4" | "ipv6"; value: string } {
  if (ipv4 && ipv6) {
    throw new Error(edgeoneCnameT("singleAddressOnly"));
  }

  if (ipv4) {
    return { family: "ipv4", value: ipv4 };
  }

  if (ipv6) {
    return { family: "ipv6", value: ipv6 };
  }

  throw new Error(edgeoneCnameT("noIpAvailable"));
}

function isValidCustomHostHeader(value: string | undefined): boolean {
  const host = normalizeDomain(value || "");
  if (!host) {
    return false;
  }

  if (
    host.includes("/") ||
    host.includes(":") ||
    host.includes("[") ||
    host.includes("]") ||
    host.includes("*") ||
    /\s/.test(host) ||
    /^https?:\/\//i.test(host)
  ) {
    return false;
  }

  if (host.length > 253) {
    return false;
  }

  return host.split(".").every((label) => {
    return (
      label.length > 0 &&
      label.length <= 63 &&
      !label.startsWith("-") &&
      !label.endsWith("-") &&
      /^[a-z0-9-]+$/i.test(label)
    );
  });
}

function isHostHeaderFormatError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return (
    message.includes("InvalidHostHeaderFormat") ||
    message.includes("HostHeaderInvalid")
  );
}

function buildOriginInfoPayload(
  desiredOrigin: { family: "ipv4" | "ipv6"; value: string },
  hostHeader?: string,
): Record<string, string> {
  return {
    OriginType: "IP_DOMAIN",
    Origin: desiredOrigin.value,
    ...(hostHeader ? { HostHeader: hostHeader } : {}),
  };
}

export async function edgeoneCnameUpdate(
  context: DDNSProviderContext,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const { config } = context;
  const secretId = config.secret_id?.trim();
  const secretKey = config.secret_key?.trim();
  const zoneId = config.zone_id?.trim();
  const domain = normalizeDomain(config.domain || "");
  if (!secretId || !secretKey || !zoneId || !domain) {
    return {
      success: false,
      message: edgeoneCnameT("configIncomplete"),
    };
  }

  let desiredOrigin: ReturnType<typeof resolveDesiredOrigin>;
  try {
    desiredOrigin = resolveDesiredOrigin(ipv4, ipv6);
  } catch (error) {
    return {
      success: false,
      message: error instanceof Error ? error.message : String(error),
    };
  }

  const list =
    await edgeOneCnameRequest<EdgeOneDescribeAccelerationDomainsResponse>(
      context,
      "DescribeAccelerationDomains",
      {
        ZoneId: zoneId,
        Offset: 0,
        Limit: 20,
        Match: "all",
        Filters: [
          {
            Name: "domain-name",
            Values: [domain],
            Fuzzy: false,
          },
        ],
      },
    );

  const existing = (list.AccelerationDomains || []).find((item) => {
    return normalizeDomain(item.DomainName || "") === domain;
  });

  if (!existing) {
    return {
      success: false,
      message: edgeoneCnameT("domainNotFound", { domain }),
    };
  }

  const originType = (existing.OriginDetail?.OriginType || "")
    .trim()
    .toUpperCase();
  if (originType && originType !== "IP_DOMAIN") {
    return {
      success: false,
      message: edgeoneCnameT("unsupportedOriginType", { originType }),
    };
  }

  const currentOrigin = existing.OriginDetail?.Origin?.trim() || "";
  const rawHostHeader = existing.OriginDetail?.HostHeader?.trim();
  const hostHeader = isValidCustomHostHeader(rawHostHeader)
    ? normalizeDomain(rawHostHeader!)
    : undefined;
  const ignoredInvalidHostHeader = Boolean(rawHostHeader) && !hostHeader;

  if (currentOrigin === desiredOrigin.value) {
    return {
      success: true,
      message: edgeoneCnameT("originUnchanged"),
      ipv4Updated: desiredOrigin.family === "ipv4",
      ipv6Updated: desiredOrigin.family === "ipv6",
    };
  }

  try {
    await edgeOneCnameRequest(context, "ModifyAccelerationDomain", {
      ZoneId: zoneId,
      DomainName: domain,
      OriginInfo: buildOriginInfoPayload(desiredOrigin, hostHeader),
    });
  } catch (error) {
    if (!hostHeader || !isHostHeaderFormatError(error)) {
      throw error;
    }

    await edgeOneCnameRequest(context, "ModifyAccelerationDomain", {
      ZoneId: zoneId,
      DomainName: domain,
      OriginInfo: buildOriginInfoPayload(desiredOrigin),
    });
  }

  return {
    success: true,
    message: ignoredInvalidHostHeader
      ? edgeoneCnameT("successWithInvalidHostHeaderIgnored")
      : edgeoneCnameT("success"),
    ipv4Updated: desiredOrigin.family === "ipv4",
    ipv6Updated: desiredOrigin.family === "ipv6",
  };
}
