import type {
  DDNSProviderContext,
  DDNSProviderDefinition,
  DDNSUpdateResult,
} from "../types";
import {
  ddnsProviderT,
  getTimeoutMs,
  normalizeDomain,
  parseJsonResponse,
  toPositiveInt,
  updateDualStack,
} from "./helpers";

const DYNU_ENDPOINT = "https://api.dynu.com/v2";
const DEFAULT_TTL = 300;
const dynuT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("dynu", key, params);
const commonT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("common", key, params);

type DynuApiEnvelope = {
  statusCode?: number;
  message?: string;
  exception?: {
    statusCode?: number;
    type?: string;
    message?: string;
  };
};

type DynuRootResponse = DynuApiEnvelope & {
  id?: number | string;
  hostname?: string;
  domainName?: string;
  node?: string;
};

type DynuDomainDetails = DynuApiEnvelope & {
  id?: number | string;
  name?: string;
  group?: string;
  ipv4Address?: string;
  ipv6Address?: string;
  ttl?: number;
  ipv4?: boolean;
  ipv6?: boolean;
  ipv4WildcardAlias?: boolean;
  ipv6WildcardAlias?: boolean;
  allowZoneTransfer?: boolean;
  dnssec?: boolean;
};

type DynuDnsRecord = {
  id?: number | string;
  domainId?: number | string;
  domainName?: string;
  nodeName?: string;
  hostname?: string;
  recordType?: string;
  ttl?: number;
  state?: boolean;
  content?: string;
  group?: string;
  ipv4Address?: string;
  ipv6Address?: string;
};

type DynuRecordListResponse = DynuApiEnvelope & {
  dnsRecords?: DynuDnsRecord[];
};

type DynuRecordPayload = {
  nodeName: string;
  recordType: "A" | "AAAA";
  ttl: number;
  state: boolean;
  group: string;
  ipv4Address?: string;
  ipv6Address?: string;
};

type DynuDomainPayload = {
  name: string;
  group: string;
  ttl: number;
  ipv4: boolean;
  ipv6: boolean;
  ipv4WildcardAlias: boolean;
  ipv6WildcardAlias: boolean;
  allowZoneTransfer: boolean;
  dnssec: boolean;
  ipv4Address?: string;
  ipv6Address?: string;
};

type DynuRoot = {
  domainId: number;
  domainName: string;
  nodeName: string;
};

export const dynuProvider: DDNSProviderDefinition = {
  name: "dynu",
  label: "Dynu",
  fields: [
    {
      key: "api_key",
      label: "API Key",
      type: "password",
      placeholder: "Dynu API Key",
      required: true,
      description: dynuT("fields.api_key.description"),
    },
    {
      key: "domain",
      label: commonT("fields.domain.label"),
      type: "text",
      placeholder: "home.example.com",
      required: true,
      description: dynuT("fields.domain.description"),
    },
    {
      key: "ttl",
      label: "TTL",
      type: "text",
      placeholder: String(DEFAULT_TTL),
      required: false,
      description: commonT("fields.ttl.description", { seconds: DEFAULT_TTL }),
    },
    {
      key: "group",
      label: "Group",
      type: "text",
      placeholder: "default",
      required: false,
      description: dynuT("fields.group.description"),
    },
  ],
};

function readPositiveId(value: unknown): number | null {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? Math.floor(parsed) : null;
}

function normalizeNodeName(value: string | null | undefined): string {
  const trimmed = value?.trim() || "";
  return trimmed === "@" ? "" : trimmed;
}

function parseConfiguredDomain(domain: string): {
  domain: string;
  wildcard: boolean;
} {
  const normalized = normalizeDomain(domain);
  if (normalized.startsWith("*.")) {
    return {
      domain: normalizeDomain(normalized.slice(2)),
      wildcard: true,
    };
  }
  return { domain: normalized, wildcard: false };
}

function buildFallbackNodeName(domain: string, rootDomain: string): string {
  const fqdn = normalizeDomain(domain);
  const normalizedRoot = normalizeDomain(rootDomain);
  if (!fqdn || !normalizedRoot || fqdn === normalizedRoot) {
    return "";
  }

  const suffix = `.${normalizedRoot}`;
  return fqdn.endsWith(suffix) ? fqdn.slice(0, -suffix.length) : "";
}

function formatDynuError(data: DynuApiEnvelope, fallback: string): string {
  if (data.exception) {
    const status = data.exception.statusCode
      ? `[${data.exception.statusCode}] `
      : "";
    const type = data.exception.type ? `${data.exception.type}: ` : "";
    return `${status}${type}${data.exception.message || fallback}`;
  }

  if (data.message?.trim()) {
    return data.message.trim();
  }

  return fallback;
}

function assertDynuSuccess(
  response: Response,
  data: DynuApiEnvelope,
  action: string,
): void {
  const fallback = dynuT("actionFailed", { action });
  if (!response.ok) {
    throw new Error(
      `[${response.status}] ${formatDynuError(data, fallback)}`,
    );
  }

  if (data.exception) {
    throw new Error(formatDynuError(data, fallback));
  }

  if (typeof data.statusCode === "number" && data.statusCode !== 200) {
    throw new Error(
      `[${data.statusCode}] ${formatDynuError(data, fallback)}`,
    );
  }
}

async function dynuRequest<T extends DynuApiEnvelope>(
  { http }: DDNSProviderContext,
  apiKey: string,
  path: string,
  options: {
    action: string;
    method?: "POST";
    body?: Record<string, unknown>;
  },
): Promise<T> {
  const headers: Record<string, string> = {
    Accept: "application/json",
    "API-Key": apiKey,
  };

  if (options.body) {
    headers["Content-Type"] = "application/json";
  }

  const response = await http.fetch(`${DYNU_ENDPOINT}${path}`, {
    ...(options.method ? { method: options.method } : {}),
    headers,
    ...(options.body ? { body: JSON.stringify(options.body) } : {}),
    signal: AbortSignal.timeout(getTimeoutMs()),
  });
  const data = await parseJsonResponse<T>(response);
  assertDynuSuccess(response, data, options.action);
  return data;
}

async function resolveDynuRoot(
  context: DDNSProviderContext,
  apiKey: string,
  domain: string,
): Promise<DynuRoot> {
  const root = await dynuRequest<DynuRootResponse>(
    context,
    apiKey,
    `/dns/getroot/${encodeURIComponent(domain)}`,
    { action: dynuT("actions.resolveRoot") },
  );
  const domainId = readPositiveId(root.id);
  const domainName = normalizeDomain(root.domainName || "");

  if (!domainId || !domainName) {
    throw new Error(dynuT("invalidRootInfo"));
  }

  return {
    domainId,
    domainName,
    nodeName:
      normalizeNodeName(root.node) || buildFallbackNodeName(domain, domainName),
  };
}

function buildRecordHostname(record: DynuDnsRecord): string {
  if (record.hostname) {
    return normalizeDomain(record.hostname);
  }

  const domainName = normalizeDomain(record.domainName || "");
  if (!domainName) {
    return "";
  }

  const nodeName = normalizeNodeName(record.nodeName);
  return nodeName ? `${nodeName}.${domainName}` : domainName;
}

function findDynuRecord(
  records: DynuDnsRecord[],
  recordType: "A" | "AAAA",
  domain: string,
  nodeName: string,
): DynuDnsRecord | null {
  const normalizedDomain = normalizeDomain(domain);
  const normalizedNodeName = normalizeNodeName(nodeName);
  const matchingType = records.filter(
    (record) => record.recordType?.toUpperCase() === recordType,
  );
  const exactHostname = matchingType.find(
    (record) => buildRecordHostname(record) === normalizedDomain,
  );
  if (exactHostname) {
    return exactHostname;
  }

  if (!normalizedNodeName) {
    return null;
  }

  return (
    matchingType.find(
      (record) => normalizeNodeName(record.nodeName) === normalizedNodeName,
    ) || null
  );
}

function getRecordAddress(
  record: DynuDnsRecord,
  recordType: "A" | "AAAA",
): string {
  return (
    (recordType === "A" ? record.ipv4Address : record.ipv6Address) ||
    record.content ||
    ""
  ).trim();
}

function buildRecordPayload(
  recordType: "A" | "AAAA",
  ip: string,
  root: DynuRoot,
  config: Record<string, string>,
  existing: DynuDnsRecord | null,
): DynuRecordPayload {
  const ttl = toPositiveInt(
    config.ttl,
    existing?.ttl && existing.ttl > 0 ? existing.ttl : DEFAULT_TTL,
  );
  const configuredGroup = config.group?.trim();
  const payload: DynuRecordPayload = {
    nodeName: root.nodeName,
    recordType,
    ttl,
    state: existing?.state ?? true,
    group: configuredGroup || existing?.group || "",
  };

  if (recordType === "A") {
    payload.ipv4Address = ip;
  } else {
    payload.ipv6Address = ip;
  }

  return payload;
}

function buildDomainPayload(
  details: DynuDomainDetails,
  domain: string,
  config: Record<string, string>,
  ipv4: string | null,
  ipv6: string | null,
): DynuDomainPayload {
  const ttl = toPositiveInt(
    config.ttl,
    details.ttl && details.ttl > 0 ? details.ttl : DEFAULT_TTL,
  );
  const payload: DynuDomainPayload = {
    name: normalizeDomain(details.name || domain),
    group: config.group?.trim() || details.group || "",
    ttl,
    ipv4: ipv4 ? true : (details.ipv4 ?? !!details.ipv4Address),
    ipv6: ipv6 ? true : (details.ipv6 ?? !!details.ipv6Address),
    ipv4WildcardAlias: ipv4 ? true : (details.ipv4WildcardAlias ?? false),
    ipv6WildcardAlias: ipv6 ? true : (details.ipv6WildcardAlias ?? false),
    allowZoneTransfer: details.allowZoneTransfer ?? false,
    dnssec: details.dnssec ?? false,
  };

  const ipv4Address = ipv4 || details.ipv4Address?.trim();
  if (ipv4Address) {
    payload.ipv4Address = ipv4Address;
  }

  const ipv6Address = ipv6 || details.ipv6Address?.trim();
  if (ipv6Address) {
    payload.ipv6Address = ipv6Address;
  }

  return payload;
}

async function updateWildcardDomain(
  context: DDNSProviderContext,
  apiKey: string,
  domain: string,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const root = await resolveDynuRoot(context, apiKey, domain);
  if (root.domainName !== domain || root.nodeName) {
    return {
      success: false,
      message: dynuT("wildcardUnsupported", { domain }),
      ipv4Updated: false,
      ipv6Updated: false,
    };
  }

  const details = await dynuRequest<DynuDomainDetails>(
    context,
    apiKey,
    `/dns/${root.domainId}`,
    { action: dynuT("actions.readDnsService") },
  );

  const ipv4Unchanged =
    !ipv4 || (details.ipv4Address === ipv4 && details.ipv4WildcardAlias);
  const ipv6Unchanged =
    !ipv6 || (details.ipv6Address === ipv6 && details.ipv6WildcardAlias);
  if (ipv4Unchanged && ipv6Unchanged) {
    return {
      success: true,
      message: dynuT("wildcardUnchanged"),
      ipv4Updated: !!ipv4,
      ipv6Updated: !!ipv6,
    };
  }

  await dynuRequest<DynuDomainDetails>(
    context,
    apiKey,
    `/dns/${root.domainId}`,
    {
      action: dynuT("actions.updateWildcardAlias"),
      method: "POST",
      body: buildDomainPayload(details, domain, context.config, ipv4, ipv6),
    },
  );

  return {
    success: true,
    message: dynuT("wildcardSuccess"),
    ipv4Updated: !!ipv4,
    ipv6Updated: !!ipv6,
  };
}

export async function dynuUpdate(
  context: DDNSProviderContext,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const { config } = context;
  const apiKey = config.api_key?.trim();
  const parsedDomain = parseConfiguredDomain(config.domain || "");
  const { domain, wildcard } = parsedDomain;

  if (!apiKey || !domain) {
    return { success: false, message: dynuT("configIncomplete") };
  }

  if (!ipv4 && !ipv6) {
    return {
      success: false,
      message: dynuT("noIpAvailable"),
    };
  }

  try {
    if (wildcard) {
      return await updateWildcardDomain(context, apiKey, domain, ipv4, ipv6);
    }

    const root = await resolveDynuRoot(context, apiKey, domain);

    return await updateDualStack("Dynu", ipv4, ipv6, async (recordType, ip) => {
      const list = await dynuRequest<DynuRecordListResponse>(
        context,
        apiKey,
        `/dns/record/${encodeURIComponent(domain)}?recordType=${recordType}`,
        { action: dynuT("actions.queryRecord", { type: recordType }) },
      );
      const existing = findDynuRecord(
        list.dnsRecords || [],
        recordType,
        domain,
        root.nodeName,
      );

      if (existing && getRecordAddress(existing, recordType) === ip) {
        return;
      }

      const payload = buildRecordPayload(
        recordType,
        ip,
        root,
        config,
        existing,
      );
      if (existing) {
        const recordId = readPositiveId(existing.id);
        if (!recordId) {
          throw new Error(dynuT("recordIdMissing"));
        }

        await dynuRequest<DynuApiEnvelope>(
          context,
          apiKey,
          `/dns/${root.domainId}/record/${recordId}`,
          {
            action: dynuT("actions.updateRecord", { type: recordType }),
            method: "POST",
            body: payload,
          },
        );
        return;
      }

      await dynuRequest<DynuApiEnvelope>(
        context,
        apiKey,
        `/dns/${root.domainId}/record`,
        {
          action: dynuT("actions.createRecord", { type: recordType }),
          method: "POST",
          body: payload,
        },
      );
    });
  } catch (error) {
    const err = error instanceof Error ? error : new Error(String(error));
    return {
      success: false,
      message: dynuT("requestError", { detail: err.message }),
      ipv4Updated: false,
      ipv6Updated: false,
    };
  }
}
