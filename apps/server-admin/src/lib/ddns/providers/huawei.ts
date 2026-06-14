import type { DDNSProviderContext, DDNSProviderDefinition, DDNSUpdateResult } from "../types";
import {
  ddnsProviderT,
  getTimeoutMs,
  parseJsonResponse,
  splitDomain,
  toPositiveInt,
  updateDualStack,
} from "./helpers";

const HUAWEI_DNS_ENDPOINT = "https://dns.myhuaweicloud.com";
const HUAWEI_AUTH_ALGORITHM = "SDK-HMAC-SHA256";
const huaweiT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("huawei", key, params);
const commonT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("common", key, params);

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

function maskSecret(value: string | undefined | null): string {
  if (!value) return "<empty>";
  if (value.length <= 6) return "***";
  return `${value.slice(0, 3)}***${value.slice(-3)}`;
}

function safeError(error: unknown): Record<string, unknown> {
  if (error instanceof Error) {
    return {
      name: error.name,
      message: error.message,
      stack: error.stack,
    };
  }
  return { value: String(error) };
}

function safeJsonParse(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

function safeJsonStringify(value: unknown): string {
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function getCrypto(): Crypto {
  if (!globalThis.crypto?.subtle) {
    throw new Error(huaweiT("webCryptoUnsupported"));
  }
  return globalThis.crypto;
}

function toHex(buffer: ArrayBuffer): string {
  return Array.from(new Uint8Array(buffer))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

async function sha256Hex(value: string): Promise<string> {
  const digest = await getCrypto().subtle.digest("SHA-256", new TextEncoder().encode(value));
  return toHex(digest);
}

async function hmacSha256Hex(key: string, value: string): Promise<string> {
  const cryptoKey = await getCrypto().subtle.importKey(
    "raw",
    new TextEncoder().encode(key),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"]
  );
  const signature = await getCrypto().subtle.sign("HMAC", cryptoKey, new TextEncoder().encode(value));
  return toHex(signature);
}

function formatHuaweiSdkDate(date = new Date()): string {
  return date.toISOString().replace(/[:-]|\.\d{3}/g, "");
}

function encodeRfc3986(value: string): string {
  return encodeURIComponent(value).replace(/[!'()*]/g, (char) =>
    `%${char.charCodeAt(0).toString(16).toUpperCase()}`
  );
}

function safeDecodeURIComponent(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function getCanonicalUri(url: URL): string {
  const pathname = url.pathname || "/";
  const encodedSegments = pathname
    .split("/")
    .map((segment) => encodeRfc3986(safeDecodeURIComponent(segment)));
  let canonicalUri = encodedSegments.join("/");
  if (!canonicalUri.startsWith("/")) canonicalUri = `/${canonicalUri}`;
  if (!canonicalUri.endsWith("/")) canonicalUri += "/";
  return canonicalUri;
}

function getCanonicalQueryString(url: URL): string {
  const params: Array<[string, string]> = [];
  url.searchParams.forEach((value, key) => {
    params.push([key, value]);
  });
  params.sort(([leftKey, leftValue], [rightKey, rightValue]) => {
    const keyCompare = encodeRfc3986(leftKey).localeCompare(encodeRfc3986(rightKey));
    if (keyCompare !== 0) return keyCompare;
    return encodeRfc3986(leftValue).localeCompare(encodeRfc3986(rightValue));
  });
  return params.map(([key, value]) => `${encodeRfc3986(key)}=${encodeRfc3986(value)}`).join("&");
}

function normalizeHeaderValue(value: string): string {
  return value.trim().replace(/\s+/g, " ");
}

async function applyHuaweiSdkAuth(
  request: Request,
  accessKeyId: string,
  secretAccessKey: string,
  payload: string,
): Promise<any> {
  const url = new URL(request.url);
  const xSdkDate = formatHuaweiSdkDate();
  request.headers.set("Content-Type", request.headers.get("Content-Type") || "application/json");
  request.headers.set("X-Sdk-Date", xSdkDate);

  const canonicalUri = getCanonicalUri(url);
  const canonicalQueryString = getCanonicalQueryString(url);
  const payloadHash = await sha256Hex(payload || "");

  const canonicalHeaderPairs: Array<[string, string]> = [
    ["content-type", normalizeHeaderValue(request.headers.get("content-type") || "application/json")],
    ["host", normalizeHeaderValue(url.host)],
    ["x-sdk-date", normalizeHeaderValue(xSdkDate)],
  ];
  canonicalHeaderPairs.sort(([left], [right]) => left.localeCompare(right));
  const canonicalHeaders = canonicalHeaderPairs.map(([key, value]) => `${key}:${value}\n`).join("");
  const signedHeaders = canonicalHeaderPairs.map(([key]) => key).join(";");

  const canonicalRequest = [
    request.method.toUpperCase(),
    canonicalUri,
    canonicalQueryString,
    canonicalHeaders,
    signedHeaders,
    payloadHash,
  ].join("\n");

  const canonicalRequestHash = await sha256Hex(canonicalRequest);
  const stringToSign = [HUAWEI_AUTH_ALGORITHM, xSdkDate, canonicalRequestHash].join("\n");
  const signature = await hmacSha256Hex(secretAccessKey, stringToSign);
  const authorization = `${HUAWEI_AUTH_ALGORITHM} Access=${accessKeyId}, SignedHeaders=${signedHeaders}, Signature=${signature}`;
  request.headers.set("Authorization", authorization);

  return { xSdkDate, canonicalUri, canonicalQueryString, signedHeaders, payloadHash };
}

export const huaweiProvider: DDNSProviderDefinition = {
  name: "huaweicloud",
  label: huaweiT("label"),
  fields: [
    { key: "access_key_id", label: "Access Key", type: "text", placeholder: huaweiT("fields.access_key_id.placeholder"), required: true },
    { key: "secret_access_key", label: "Secret Key", type: "password", placeholder: huaweiT("fields.secret_access_key.placeholder"), required: true },
    { key: "root_domain", label: commonT("fields.root_domain.label"), type: "text", placeholder: "example.com", required: true },
    { key: "domain", label: commonT("fields.domain.label"), type: "text", placeholder: "home.example.com", required: true },
    { key: "ttl", label: "TTL", type: "text", placeholder: "300", required: false, description: commonT("fields.ttl.description", { seconds: 300 }) },
  ],
};

async function huaweiRequest<T>(
  context: DDNSProviderContext,
  path: string,
  method: "GET" | "POST" | "PUT",
  body?: Record<string, unknown>,
): Promise<T> {
  const { config, http } = context;
  const accessKeyId = config.access_key_id;
  const secretAccessKey = config.secret_access_key;

  if (!accessKeyId || !secretAccessKey) throw new Error(huaweiT("configIncomplete"));

  const payload = body ? JSON.stringify(body) : "";
  const request = new Request(`${HUAWEI_DNS_ENDPOINT}${path}`, {
    method,
    headers: { "Content-Type": "application/json" },
    body: payload || undefined,
    signal: AbortSignal.timeout(getTimeoutMs()),
  });

  await applyHuaweiSdkAuth(request, accessKeyId, secretAccessKey, payload);
  const response = await http.fetch(request);

  if (!response.ok) {
    const text = await response.text();
    throw new Error(
      huaweiT("requestFailed", {
        status: response.status,
        statusText: response.statusText,
        detail: safeJsonStringify(safeJsonParse(text)),
      }),
    );
  }

  return parseJsonResponse<T>(response);
}

export async function huaweiUpdate(
  context: DDNSProviderContext,
  ipv4: string | null,
  ipv6: string | null,
): Promise<DDNSUpdateResult> {
  const { config } = context;
  const { access_key_id, secret_access_key, root_domain, domain } = config;

  if (!access_key_id || !secret_access_key || !root_domain || !domain) {
    return { success: false, message: huaweiT("configIncomplete") };
  }

  const ttl = toPositiveInt(config.ttl, 300);
  const parsed = splitDomain(domain, root_domain);
  const normalizedRootDomain = parsed.rootDomain.replace(/\.+$/, "");
  const normalizedFqdn = parsed.fqdn.replace(/\.+$/, "");
  const expectedZoneName = `${normalizedRootDomain}.`;
  const fqdnWithDot = `${normalizedFqdn}.`;

  const zoneResponse = await huaweiRequest<HuaweiZoneResponse>(
    context,
    `/v2/zones?name=${normalizedRootDomain}`,
    "GET"
  );
  const zone = zoneResponse.zones?.find((item) => item.name === expectedZoneName);
  if (!zone) return { success: false, message: huaweiT("zoneNotFound", { zone: expectedZoneName }) };

  return updateDualStack(huaweiT("label"), ipv4, ipv6, async (recordType, ip) => {
    const recordsetPath = `/v2/zones/${encodeURIComponent(zone.id)}/recordsets?search_mode=equal&type=${recordType}&name=${fqdnWithDot}&limit=500`;
    const records = await huaweiRequest<HuaweiRecordsetListResponse>(context, recordsetPath, "GET");
    const existing = (records.recordsets || []).find(
      (record) => record.zone_id === zone.id && record.name === fqdnWithDot && record.type === recordType
    );

    if (existing) {
      const isSameRecord = existing.records.length === 1 && existing.records[0] === ip && existing.ttl === ttl;
      if (isSameRecord) return;

      await huaweiRequest<HuaweiRecordset>(
        context,
        `/v2.1/zones/${encodeURIComponent(zone.id)}/recordsets/${encodeURIComponent(existing.id)}`,
        "PUT",
        { name: fqdnWithDot, type: recordType, ttl, records: [ip] }
      );
      return;
    }

    await huaweiRequest<HuaweiRecordset>(
      context,
      `/v2/zones/${encodeURIComponent(zone.id)}/recordsets`,
      "POST",
      { name: fqdnWithDot, type: recordType, ttl, records: [ip] }
    );
  });
}
