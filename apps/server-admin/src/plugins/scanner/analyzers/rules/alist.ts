import { AnalyzerRule, ScanResult } from "../../types";

interface PublicSettingsResponse {
  code?: number;
  data?: {
    site_title?: string;
    version?: string;
  };
}

const listSettingsCache = new WeakMap<ScanResult, Promise<PublicSettingsResponse | null>>();

function hasListTitle(body?: string): boolean {
  if (!body) return false;

  const match = body.match(/<title[^>]*>(.*?)<\/title>/i);
  const title = match?.[1]?.trim().toLowerCase() ?? "";
  return title.includes("list");
}

async function getPublicSettings(result: ScanResult): Promise<PublicSettingsResponse | null> {
  if (!hasListTitle(result.body)) {
    return null;
  }

  const cached = listSettingsCache.get(result);
  if (cached) {
    return cached;
  }

  const request = fetch(`http://${result.host}:${result.port}/api/public/settings`, {
    signal: AbortSignal.timeout(2000),
    headers: {
      "User-Agent": "Node-Elysia-Scanner/1.0",
      Connection: "close",
      Accept: "application/json",
    },
  })
    .then(async (response) => {
      if (!response.ok) {
        return null;
      }

      const payload = await response.json() as PublicSettingsResponse;
      if (payload?.code !== 200 || !payload.data) {
        return null;
      }

      return payload;
    })
    .catch(() => null);

  listSettingsCache.set(result, request);
  return request;
}

export const xiaoyaRule: AnalyzerRule = {
  name: "xiaoya",
  label: "小雅Alist",
  rule: {
    path: "/xiaoya",
    rewrite_html: false,
    use_auth: true,
    use_root_mode: true,
    strip_path: true,
    target: "",
  },
  isDefault: false,
  match: async (result) => {
    const settings = await getPublicSettings(result);
    return settings?.data?.site_title === "小雅的分类 Alist";
  },
};

export const alistRule: AnalyzerRule = {
  name: "alist",
  label: "AList",
  rule: {
    path: "/alist",
    rewrite_html: false,
    use_auth: true,
    use_root_mode: true,
    strip_path: true,
    target: "",
  },
  isDefault: false,
  match: async (result) => {
    const settings = await getPublicSettings(result);
    return settings?.data?.site_title === "Alist";
  },
};

export const openListRule: AnalyzerRule = {
  name: "openlist",
  label: "OpenList",
  rule: {
    path: "/openlist",
    rewrite_html: false,
    use_auth: true,
    use_root_mode: true,
    strip_path: true,
    target: "",
  },
  isDefault: false,
  match: async (result) => {
    const settings = await getPublicSettings(result);
    return settings?.data?.site_title === "OpenList";
  },
};
