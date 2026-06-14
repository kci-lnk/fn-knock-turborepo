import type { DDNSProviderContext } from "../types";
import {
  ddnsProviderT,
  normalizeDomain,
  requestTencentCloudJson,
} from "./helpers";

const edgeoneT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("edgeone", key, params);

export const EDGEONE_API_HOST = "teo.tencentcloudapi.com";
export const EDGEONE_API_VERSION = "2022-09-01";
export const EDGEONE_SERVICE = "teo";

export const EDGEONE_OVERSEAS_ACCESS_MODE_FIELD = "edgeone_overseas_access";
export const DEFAULT_EDGEONE_OVERSEAS_ACCESS_MODE = "off";
export const EDGEONE_ALLOWED_MAINLAND_AREA_CODES = [
  "CN",
  "TW",
  "HK",
  "MO",
] as const;
export const EDGEONE_ALLOWED_MAINLAND_REGION_CODES = [
  "CN-Other",
  "CN-SH",
  "CN-YN",
  "CN-NM",
  "CN-BJ",
  "CN-JL",
  "CN-SC",
  "CN-TJ",
  "CN-NX",
  "CN-AH",
  "CN-SD",
  "CN-GD",
  "CN-GX",
  "CN-XJ",
  "CN-JS",
  "CN-JX",
  "CN-HE",
  "CN-HA",
  "CN-ZJ",
  "CN-HI",
  "CN-HB",
  "CN-HN",
  "CN-GS",
  "CN-FJ",
  "CN-XZ",
  "CN-GZ",
  "CN-LN",
  "CN-CQ",
  "CN-SN",
  "CN-QH",
  "CN-HL",
  "CN-SX",
  "CN-MO",
  "CN-TW",
  "CN-HK",
] as const;

export type EdgeOneOverseasAccessMode = "off" | "block_overseas";

export function isEdgeOneDDNSProvider(providerName: string): boolean {
  return providerName === "edgeone" || providerName === "edgeone_cname";
}

export function normalizeEdgeOneOverseasAccessMode(
  value: string | null | undefined,
): EdgeOneOverseasAccessMode {
  return value === "block_overseas"
    ? "block_overseas"
    : DEFAULT_EDGEONE_OVERSEAS_ACCESS_MODE;
}

export function resolveEdgeOneApiHost(endpoint: string | undefined): string {
  const value = endpoint?.trim();
  if (!value) {
    return EDGEONE_API_HOST;
  }

  if (/^https?:\/\//i.test(value)) {
    return new URL(value).host || EDGEONE_API_HOST;
  }

  return value.replace(/\/+$/, "") || EDGEONE_API_HOST;
}

export function resolveEdgeOneRegion(
  region: string | undefined,
): string | undefined {
  const value = region?.trim();
  return value || undefined;
}

export function getEdgeOneDomainTarget(config: Record<string, string>): {
  domain: string;
  endpointHost: string;
  region: string | null;
  zoneId: string;
} {
  const zoneId = config.zone_id?.trim();
  const domain = normalizeDomain(config.domain || "");
  if (!zoneId || !domain) {
    throw new Error(edgeoneT("configTargetIncomplete"));
  }

  return {
    zoneId,
    domain,
    endpointHost: resolveEdgeOneApiHost(config.endpoint),
    region: resolveEdgeOneRegion(config.region) || null,
  };
}

export async function requestEdgeOneJson<T>(
  context: DDNSProviderContext,
  action: string,
  payload: Record<string, unknown>,
): Promise<T> {
  const { config, http } = context;
  const secretId = config.secret_id?.trim();
  const secretKey = config.secret_key?.trim();
  if (!secretId || !secretKey) {
    throw new Error(edgeoneT("configIncomplete"));
  }

  return requestTencentCloudJson<T>(http, {
    action,
    host: resolveEdgeOneApiHost(config.endpoint),
    payload,
    region: resolveEdgeOneRegion(config.region),
    secretId,
    secretKey,
    service: EDGEONE_SERVICE,
    version: EDGEONE_API_VERSION,
  });
}
