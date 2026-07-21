export const advancedAuthRequestHeaderGroups = [
  {
    id: "standard",
    labelKey: "admin.advancedAuth.headerGroups.standard",
    headers: [
      "Accept",
      "Accept-Charset",
      "Accept-Encoding",
      "Accept-Language",
      "Cache-Control",
      "Content-Length",
      "Content-Type",
      "If-Match",
      "If-Modified-Since",
      "If-None-Match",
      "If-Unmodified-Since",
      "Origin",
      "Pragma",
      "Range",
      "Referer",
      "User-Agent",
    ],
  },
  {
    id: "fetch_metadata",
    labelKey: "admin.advancedAuth.headerGroups.fetchMetadata",
    headers: [
      "Sec-Fetch-Dest",
      "Sec-Fetch-Mode",
      "Sec-Fetch-Site",
      "Sec-Fetch-User",
    ],
  },
  {
    id: "application",
    labelKey: "admin.advancedAuth.headerGroups.application",
    headers: [
      "X-Api-Key",
      "X-Auth-Token",
      "X-Client-Id",
      "X-Correlation-Id",
      "X-Device-Id",
      "X-Request-Id",
      "X-Requested-With",
      "X-Tenant-Id",
      "X-User-Id",
    ],
  },
] as const;

export const advancedAuthRequestHeaderNames =
  advancedAuthRequestHeaderGroups.flatMap((group) => group.headers);
