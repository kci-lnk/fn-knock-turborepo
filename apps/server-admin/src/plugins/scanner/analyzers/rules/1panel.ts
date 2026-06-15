import { AnalyzerRule, ScanResult } from "../../types";

const faviconCache = new WeakMap<ScanResult, Promise<boolean>>();

function hasLoadingTitle(body?: string): boolean {
  if (!body) return false;

  const match = body.match(/<title[^>]*>([\s\S]*?)<\/title>/i);
  return match?.[1]?.trim().toLowerCase() === "loading...";
}

async function hasPublicFavicon(result: ScanResult): Promise<boolean> {
  if (!hasLoadingTitle(result.body)) {
    return false;
  }

  const cached = faviconCache.get(result);
  if (cached) {
    return cached;
  }

  const request = fetch(
    `http://${result.host}:${result.port}/public/favicon.png`,
    {
      signal: AbortSignal.timeout(2000),
      headers: {
        "User-Agent": "Node-Elysia-Scanner/1.0",
        Connection: "close",
        Accept: "image/*,*/*;q=0.8",
      },
    },
  )
    .then((response) => {
      if (!response.ok) return false;

      const contentType = response.headers
        .get("content-type")
        ?.split(";")[0]
        ?.trim()
        ?.toLowerCase();
      return (
        !contentType ||
        contentType.startsWith("image/") ||
        contentType === "application/octet-stream" ||
        contentType === "binary/octet-stream"
      );
    })
    .catch(() => false);

  faviconCache.set(result, request);
  return request;
}

export const onePanelRule: AnalyzerRule = {
  name: "1Panel",
  label: "1Panel",
  rule: {
    path: "/1panel",
    rewrite_html: false,
    use_auth: true,
    use_root_mode: true,
    strip_path: true,
    target: "",
  },
  isDefault: false,
  match: hasPublicFavicon,
};
