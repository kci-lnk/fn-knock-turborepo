import type { HostLocation } from "@/types";

export type GatewayLocationHeaderRow = {
  name: string;
  value: string;
};

export type GatewayLocationForm = Omit<HostLocation, "response"> & {
  response: HostLocation["response"];
  headers: GatewayLocationHeaderRow[];
};

export const DEFAULT_RESPONSE_CONTENT_TYPE = "text/plain; charset=utf-8";

export const createDefaultLocation = (): HostLocation => ({
  path: "",
  match: "exact",
  action: "proxy",
  target: "",
  strip_path: true,
  rewrite_html: true,
  response: {
    status: 200,
    content_type: DEFAULT_RESPONSE_CONTENT_TYPE,
    headers: {},
    body: "",
  },
});

export const createDefaultLocationForm = (): GatewayLocationForm => ({
  ...createDefaultLocation(),
  headers: [],
});

export const cloneLocation = (location: HostLocation): HostLocation => ({
  ...location,
  response: {
    status: location.response?.status ?? 200,
    content_type:
      location.response?.content_type?.trim() || DEFAULT_RESPONSE_CONTENT_TYPE,
    headers: { ...(location.response?.headers ?? {}) },
    body: location.response?.body ?? "",
  },
});

export const cleanHostLocationPath = (value: string): string => {
  const raw = value.trim();
  if (!raw.startsWith("/")) return raw;

  const segments: string[] = [];
  for (const segment of raw.split("/")) {
    if (!segment || segment === ".") continue;
    if (segment === "..") {
      segments.pop();
      continue;
    }
    segments.push(segment);
  }

  return `/${segments.join("/")}`;
};
