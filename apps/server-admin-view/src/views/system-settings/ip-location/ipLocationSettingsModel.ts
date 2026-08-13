import { type IpLocationApiConfig } from "@/lib/api/config";

export const OFFICIAL_IP_LOOKUP_URL = "https://ipaddress.fnknock.cn/api/v1";
export const OFFICIAL_CIDR_URL = "https://cidr.fnknock.cn/api/v1";
export const DEFAULT_CUSTOM_IP_LOOKUP_URL = "http://127.0.0.1:30661";
export const DEFAULT_CUSTOM_CIDR_URL = "http://127.0.0.1:30662";

export const normalizeIpLocationBaseUrl = (value: string) =>
  value.trim().replace(/\/+$/, "");

export const isHttpUrl = (value: string) => {
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
};

export const buildIpLocationSettingsPayload = ({
  cidrMode,
  cidrUrl,
  ipLookupMode,
  ipLookupUrl,
}: {
  cidrMode: IpLocationApiConfig["cidr_mode"];
  cidrUrl: string;
  ipLookupMode: IpLocationApiConfig["ip_lookup_mode"];
  ipLookupUrl: string;
}): IpLocationApiConfig => ({
  ip_lookup_mode: ipLookupMode,
  ip_lookup_url:
    ipLookupMode === "custom"
      ? normalizeIpLocationBaseUrl(ipLookupUrl)
      : OFFICIAL_IP_LOOKUP_URL,
  cidr_mode: cidrMode,
  cidr_url:
    cidrMode === "custom"
      ? normalizeIpLocationBaseUrl(cidrUrl)
      : OFFICIAL_CIDR_URL,
});

export const normalizeIpLocationSettings = (
  settings: IpLocationApiConfig,
): IpLocationApiConfig => ({
  ip_lookup_mode: settings.ip_lookup_mode,
  ip_lookup_url: normalizeIpLocationBaseUrl(settings.ip_lookup_url),
  cidr_mode: settings.cidr_mode,
  cidr_url: normalizeIpLocationBaseUrl(settings.cidr_url),
});
