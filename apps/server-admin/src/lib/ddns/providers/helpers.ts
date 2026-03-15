import { createHash, createHmac, randomUUID } from "node:crypto";
import type { DDNSUpdateResult, DDNSUpdateScope } from "../types";

const DEFAULT_TIMEOUT_MS = 10_000;
export const DDNS_UPDATE_SCOPE_FIELD = "update_scope";
export const DEFAULT_DDNS_UPDATE_SCOPE: DDNSUpdateScope = "dual_stack";

export function getTimeoutMs(): number {
  const value = Number(process.env.DDNS_TIMEOUT_MS || DEFAULT_TIMEOUT_MS);
  return Number.isFinite(value) && value > 0 ? value : DEFAULT_TIMEOUT_MS;
}

export function normalizeUpdateScope(value: string | null | undefined): DDNSUpdateScope {
  if (value === "ipv6_only" || value === "ipv4_only" || value === "dual_stack") {
    return value;
  }
  return DEFAULT_DDNS_UPDATE_SCOPE;
}

export function applyUpdateScope(
  scope: DDNSUpdateScope,
  ipv4: string | null,
  ipv6: string | null,
): { ipv4: string | null; ipv6: string | null } {
  return {
    ipv4: scope === "ipv6_only" ? null : ipv4,
    ipv6: scope === "ipv4_only" ? null : ipv6,
  };
}

export function getUpdateScopeUnavailableMessage(scope: DDNSUpdateScope): string {
  if (scope === "ipv6_only") {
    return "当前更新范围为仅更新 IPv6，但未检测到可用的 IPv6 地址";
  }
  if (scope === "ipv4_only") {
    return "当前更新范围为仅更新 IPv4，但未检测到可用的 IPv4 地址";
  }
  return "当前更新范围内没有可用的 IPv4 或 IPv6 地址";
}

export function toPositiveInt(value: string | undefined, fallback: number): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? Math.floor(parsed) : fallback;
}

export function normalizeDomain(value: string): string {
  return value.trim().replace(/\.+$/, "");
}

export function splitDomain(fullDomain: string, rootDomain: string): {
  fqdn: string;
  rootDomain: string;
  recordName: string;
} {
  const fqdn = normalizeDomain(fullDomain);
  const zone = normalizeDomain(rootDomain);

  if (!fqdn || !zone) {
    throw new Error("域名配置不完整");
  }

  if (fqdn === zone) {
    return { fqdn, rootDomain: zone, recordName: "@" };
  }

  const suffix = `.${zone}`;
  if (!fqdn.endsWith(suffix)) {
    throw new Error(`域名 ${fqdn} 不属于根域 ${zone}`);
  }

  return {
    fqdn,
    rootDomain: zone,
    recordName: fqdn.slice(0, -suffix.length),
  };
}

export async function parseJsonResponse<T>(response: Response): Promise<T> {
  const text = await response.text();
  if (!text) {
    return {} as T;
  }

  try {
    return JSON.parse(text) as T;
  } catch {
    throw new Error(`响应不是合法 JSON: ${text}`);
  }
}

export async function parseTextResponse(response: Response): Promise<string> {
  return (await response.text()).trim();
}

export async function updateDualStack(
  providerLabel: string,
  ipv4: string | null,
  ipv6: string | null,
  updateRecord: (recordType: "A" | "AAAA", ip: string) => Promise<void>,
): Promise<DDNSUpdateResult> {
  let ipv4Updated = false;
  let ipv6Updated = false;
  const errors: string[] = [];

  if (ipv4) {
    try {
      await updateRecord("A", ipv4);
      ipv4Updated = true;
    } catch (error) {
      errors.push(formatError(`A 记录处理失败`, error));
    }
  }

  if (ipv6) {
    try {
      await updateRecord("AAAA", ipv6);
      ipv6Updated = true;
    } catch (error) {
      errors.push(formatError(`AAAA 记录处理失败`, error));
    }
  }

  if (errors.length > 0) {
    return {
      success: false,
      message: errors.join("; "),
      ipv4Updated,
      ipv6Updated,
    };
  }

  return {
    success: true,
    message: `${providerLabel} DNS 更新成功`,
    ipv4Updated,
    ipv6Updated,
  };
}

export function formatError(prefix: string, error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  return `${prefix}: ${message}`;
}

export function rfc3986Encode(value: string): string {
  return encodeURIComponent(value)
    .replace(/[!'()*]/g, (char) => `%${char.charCodeAt(0).toString(16).toUpperCase()}`);
}

export function sha256Hex(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

export function hmacHex(algorithm: "sha1" | "sha256", key: string, value: string): string {
  return createHmac(algorithm, key).update(value).digest("hex");
}

export function hmacBase64(algorithm: "sha1" | "sha256", key: string, value: string): string {
  return createHmac(algorithm, key).update(value).digest("base64");
}

export function formatIso8601Utc(date: Date): string {
  return date.toISOString().replace(/\.\d{3}Z$/, "Z");
}

export function formatCompactUtc(date: Date): string {
  return date.toISOString().replace(/[-:]/g, "").replace(/\.\d{3}Z$/, "Z");
}

export function buildAliyunSignedParams(
  accessKeyId: string,
  accessKeySecret: string,
  extraParams: Record<string, string>,
): URLSearchParams {
  const params = new Map<string, string>([
    ["AccessKeyId", accessKeyId],
    ["Format", "JSON"],
    ["SignatureMethod", "HMAC-SHA1"],
    ["SignatureNonce", randomUUID()],
    ["SignatureVersion", "1.0"],
    ["Timestamp", formatIso8601Utc(new Date())],
    ["Version", "2015-01-09"],
  ]);

  for (const [key, value] of Object.entries(extraParams)) {
    params.set(key, value);
  }

  const sorted = [...params.entries()].sort(([a], [b]) => a.localeCompare(b));
  const canonicalized = sorted
    .map(([key, value]) => `${rfc3986Encode(key)}=${rfc3986Encode(value)}`)
    .join("&");
  const stringToSign = `GET&${rfc3986Encode("/")}&${rfc3986Encode(canonicalized)}`;
  const signature = hmacBase64("sha1", `${accessKeySecret}&`, stringToSign);

  const search = new URLSearchParams();
  for (const [key, value] of sorted) {
    search.set(key, value);
  }
  search.set("Signature", signature);
  return search;
}

function buildCanonicalQuery(params: URLSearchParams): string {
  return [...params.entries()]
    .sort(([aKey, aValue], [bKey, bValue]) => {
      const keyOrder = aKey.localeCompare(bKey);
      return keyOrder !== 0 ? keyOrder : aValue.localeCompare(bValue);
    })
    .map(([key, value]) => `${rfc3986Encode(key)}=${rfc3986Encode(value)}`)
    .join("&");
}

export function applyBaiduBceAuth(
  request: Request,
  accessKeyId: string,
  secretAccessKey: string,
): void {
  const url = new URL(request.url);
  const timestamp = formatIso8601Utc(new Date());
  request.headers.set("Host", url.host);
  request.headers.set("x-bce-date", timestamp);

  if (!request.headers.has("Content-Type")) {
    request.headers.set("Content-Type", "application/json");
  }

  const signedHeaderNames = ["content-type", "host", "x-bce-date"];
  const canonicalHeaders = signedHeaderNames
    .map((name) => {
      const value = request.headers.get(name) ?? request.headers.get(name.toLowerCase()) ?? "";
      return `${name}:${rfc3986Encode(value.trim())}`;
    })
    .join("\n");

  const authStringPrefix = `bce-auth-v1/${accessKeyId}/${timestamp}/1800`;
  const signingKey = createHmac("sha256", secretAccessKey).update(authStringPrefix).digest("hex");
  const canonicalRequest = [
    request.method.toUpperCase(),
    url.pathname || "/",
    buildCanonicalQuery(url.searchParams),
    canonicalHeaders,
  ].join("\n");
  const signature = hmacHex("sha256", signingKey, canonicalRequest);

  request.headers.set(
    "Authorization",
    `${authStringPrefix}/${signedHeaderNames.join(";")}/${signature}`,
  );
}

export function applyHuaweiSdkAuth(
  request: Request,
  accessKeyId: string,
  secretAccessKey: string,
  body: string,
): void {
  const url = new URL(request.url);
  const timestamp = formatCompactUtc(new Date());
  request.headers.set("Host", url.host);
  request.headers.set("X-Sdk-Date", timestamp);

  if (!request.headers.has("Content-Type")) {
    request.headers.set("Content-Type", "application/json");
  }

  const signedHeaderNames = ["content-type", "host", "x-sdk-date"];
  const canonicalHeaders = signedHeaderNames
    .map((name) => `${name}:${(request.headers.get(name) || "").trim()}`)
    .join("\n");
  const canonicalRequest = [
    request.method.toUpperCase(),
    url.pathname || "/",
    buildCanonicalQuery(url.searchParams),
    `${canonicalHeaders}\n`,
    signedHeaderNames.join(";"),
    sha256Hex(body),
  ].join("\n");
  const stringToSign = `SDK-HMAC-SHA256\n${timestamp}\n${sha256Hex(canonicalRequest)}`;
  const signature = hmacHex("sha256", secretAccessKey, stringToSign);

  request.headers.set(
    "Authorization",
    `SDK-HMAC-SHA256 Access=${accessKeyId}, SignedHeaders=${signedHeaderNames.join(";")}, Signature=${signature}`,
  );
}

export function parseHeaderLines(value: string): Record<string, string> {
  const headers: Record<string, string> = {};
  for (const line of value.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const separatorIndex = trimmed.indexOf(":");
    if (separatorIndex <= 0) {
      throw new Error(`无效 Header 格式: ${trimmed}`);
    }
    const key = trimmed.slice(0, separatorIndex).trim();
    const headerValue = trimmed.slice(separatorIndex + 1).trim();
    headers[key] = headerValue;
  }
  return headers;
}

export function applyTemplate(template: string, values: Record<string, string>): string {
  return template.replace(/#\{([^}]+)\}/g, (_, key: string) => values[key] ?? "");
}
