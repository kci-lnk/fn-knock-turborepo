import type {
  DDNSProviderContext,
  DDNSProviderDefinition,
  DDNSUpdateResult,
} from "../types";
import {
  ddnsProviderT,
  normalizeDomain,
  toPositiveInt,
  updateDualStack,
} from "./helpers";
import {
  EDGEONE_OVERSEAS_ACCESS_MODE_FIELD,
  requestEdgeOneJson,
} from "./edgeone-shared";

const edgeoneT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("edgeone", key, params);
const commonT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("common", key, params);

type EdgeOneDnsRecord = {
  RecordId?: string;
  Name?: string;
  Type?: string;
  Location?: string;
  Content?: string;
  TTL?: number;
};

type EdgeOneDescribeDnsRecordsResponse = {
  DnsRecords?: EdgeOneDnsRecord[];
  TotalCount?: number;
};

type EdgeOneCreateDnsRecordResponse = {
  RecordId?: string;
};

export const edgeoneProvider: DDNSProviderDefinition = {
  name: "edgeone",
  label: edgeoneT("label"),
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
      placeholder: edgeoneT("fields.secret_key.placeholder"),
      required: true,
    },
    {
      key: "zone_id",
      label: "Zone ID",
      type: "text",
      placeholder: "zone-xxxxxxxx",
      required: true,
      description: edgeoneT("fields.zone_id.description"),
    },
    {
      key: "domain",
      label: commonT("fields.domain.label"),
      type: "text",
      placeholder: "home.example.com",
      required: true,
      description: edgeoneT("fields.domain.description"),
    },
    {
      key: "location",
      label: edgeoneT("fields.location.label"),
      type: "text",
      placeholder: edgeoneT("fields.location.placeholder"),
      required: false,
      description: edgeoneT("fields.location.description"),
    },
    {
      key: "ttl",
      label: "TTL",
      type: "text",
      placeholder: "300",
      required: false,
      description: edgeoneT("fields.ttl.description"),
    },
    {
      key: EDGEONE_OVERSEAS_ACCESS_MODE_FIELD,
      label: edgeoneT("fields.overseas_access.label"),
      type: "select",
      required: false,
      options: [
        { label: edgeoneT("fields.overseas_access.options.off"), value: "off" },
        {
          label: edgeoneT("fields.overseas_access.options.blockOverseas"),
          value: "block_overseas",
        },
      ],
      description: edgeoneT("fields.overseas_access.description"),
    },
    {
      key: "endpoint",
      label: "API Endpoint",
      type: "text",
      placeholder: "https://teo.tencentcloudapi.com",
      required: false,
      description: edgeoneT("fields.endpoint.description"),
    },
    {
      key: "region",
      label: "Region",
      type: "text",
      placeholder: edgeoneT("fields.region.placeholder"),
      required: false,
      description: edgeoneT("fields.region.description"),
    },
  ],
};

function normalizeEdgeOneLocation(value: string | undefined): string {
  const trimmed = value?.trim();
  if (!trimmed) {
    return "default";
  }
  return trimmed.toLowerCase();
}

async function edgeoneRequest<T>(
  context: DDNSProviderContext,
  action: string,
  payload: Record<string, unknown>,
): Promise<T> {
  const { config } = context;
  const secretId = config.secret_id?.trim();
  const secretKey = config.secret_key?.trim();
  if (!secretId || !secretKey) {
    throw new Error(edgeoneT("configIncomplete"));
  }

  return requestEdgeOneJson<T>(context, action, payload);
}

export async function edgeoneUpdate(
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
    return { success: false, message: edgeoneT("configIncomplete") };
  }

  const ttl = toPositiveInt(config.ttl, 300);
  const desiredLocation = normalizeEdgeOneLocation(config.location);

  return updateDualStack(
    edgeoneT("label"),
    ipv4,
    ipv6,
    async (recordType, ip) => {
      const list = await edgeoneRequest<EdgeOneDescribeDnsRecordsResponse>(
        context,
        "DescribeDnsRecords",
        {
          ZoneId: zoneId,
          Offset: 0,
          Limit: 100,
          Match: "all",
          Filters: [
            {
              Name: "name",
              Values: [domain],
              Fuzzy: false,
            },
          ],
        },
      );

      const existing = (list.DnsRecords || []).find((record) => {
        return (
          normalizeDomain(record.Name || "") === domain &&
          (record.Type || "").toUpperCase() === recordType &&
          normalizeEdgeOneLocation(record.Location) === desiredLocation
        );
      });

      if (existing) {
        if (existing.Content === ip) {
          return;
        }

        if (!existing.RecordId) {
          throw new Error(edgeoneT("missingRecordId"));
        }

        await edgeoneRequest(context, "ModifyDnsRecords", {
          ZoneId: zoneId,
          DnsRecords: [
            {
              RecordId: existing.RecordId,
              Name: domain,
              Type: recordType,
              Content: ip,
              TTL: ttl,
              ...(desiredLocation !== "default"
                ? { Location: config.location?.trim() }
                : {}),
            },
          ],
        });
        return;
      }

      const result = await edgeoneRequest<EdgeOneCreateDnsRecordResponse>(
        context,
        "CreateDnsRecord",
        {
          ZoneId: zoneId,
          Name: domain,
          Type: recordType,
          Content: ip,
          TTL: ttl,
          ...(desiredLocation !== "default"
            ? { Location: config.location?.trim() }
            : {}),
        },
      );

      if (!result.RecordId) {
        throw new Error(edgeoneT("missingCreatedRecordId"));
      }
    },
  );
}
