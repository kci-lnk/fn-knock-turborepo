import { Buffer } from "node:buffer";

const DEFAULT_REQUEST_TIMEOUT_MS = 5000;
const MAX_HTML_LENGTH = 256 * 1024;
const MAX_FAVICON_BYTES = 128 * 1024;
const METADATA_USER_AGENT = "fn-knock-server-admin/1.0";

export interface UrlMetadata {
  title: string;
  favicon: string;
  finalUrl: string;
}

export interface UrlMetadataResult {
  ok: boolean;
  data: UrlMetadata;
  error?: string;
}

const collapseWhitespace = (value: string): string =>
  value.replace(/\s+/g, " ").trim();

const decodeHtmlEntities = (value: string): string =>
  value.replace(
    /&(#x?[0-9a-f]+|amp|lt|gt|quot|apos|nbsp);/gi,
    (entity, token: string) => {
      const normalized = token.toLowerCase();
      switch (normalized) {
        case "amp":
          return "&";
        case "lt":
          return "<";
        case "gt":
          return ">";
        case "quot":
          return '"';
        case "apos":
          return "'";
        case "nbsp":
          return " ";
      }

      if (normalized.startsWith("#x")) {
        const codePoint = Number.parseInt(normalized.slice(2), 16);
        return Number.isFinite(codePoint)
          ? String.fromCodePoint(codePoint)
          : entity;
      }

      if (normalized.startsWith("#")) {
        const codePoint = Number.parseInt(normalized.slice(1), 10);
        return Number.isFinite(codePoint)
          ? String.fromCodePoint(codePoint)
          : entity;
      }

      return entity;
    },
  );

const parseHtmlAttributes = (tag: string): Record<string, string> => {
  const attributes: Record<string, string> = {};
  const attributeRegex =
    /([^\s=/>]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+)))?/g;
  let match: RegExpExecArray | null = null;

  while ((match = attributeRegex.exec(tag))) {
    const [, rawName, doubleQuoted, singleQuoted, bareValue] = match;
    if (!rawName) continue;
    const name = rawName.toLowerCase();
    const value = doubleQuoted ?? singleQuoted ?? bareValue ?? "";
    attributes[name] = value;
  }

  return attributes;
};

const getFaviconPriority = (rel: string): number => {
  const normalized = rel.trim().toLowerCase().replace(/\s+/g, " ");
  if (!normalized) return 0;
  if (normalized === "icon") return 500;
  if (normalized === "shortcut icon") return 450;
  if (normalized.includes("apple-touch-icon")) return 400;
  if (normalized.includes("mask-icon")) return 300;
  if (normalized.split(" ").includes("icon")) return 350;
  return 0;
};

export const normalizeHttpUrl = (value: string): string => {
  const trimmed = value.trim();
  if (!trimmed) return "";

  try {
    const parsed = new URL(trimmed);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return "";
    }
    return parsed.toString();
  } catch {
    return "";
  }
};

const normalizeFaviconUrl = (value: string, baseUrl: string): string => {
  const trimmed = value.trim();
  if (!trimmed) return "";
  if (/^data:image\//i.test(trimmed)) {
    return trimmed;
  }

  try {
    const resolved = new URL(trimmed, baseUrl);
    if (
      resolved.protocol !== "http:" &&
      resolved.protocol !== "https:" &&
      resolved.protocol !== "data:"
    ) {
      return "";
    }
    return resolved.toString();
  } catch {
    return "";
  }
};

const resolveDefaultFaviconUrl = (value: string): string => {
  try {
    const parsed = new URL(value);
    return `${parsed.origin}/favicon.ico`;
  } catch {
    return "";
  }
};

export const extractTitleFromHtml = (html: string): string => {
  const match = html.match(/<title\b[^>]*>([\s\S]*?)<\/title>/i);
  return collapseWhitespace(decodeHtmlEntities(match?.[1] ?? ""));
};

export const extractFaviconFromHtml = (
  html: string,
  baseUrl: string,
): string => {
  const linkTags = html.match(/<link\b[^>]*>/gi) ?? [];
  let best: { href: string; priority: number } | null = null;

  for (const tag of linkTags) {
    const attributes = parseHtmlAttributes(tag);
    const priority = getFaviconPriority(attributes.rel ?? "");
    if (priority <= 0) continue;

    const href = normalizeFaviconUrl(attributes.href ?? "", baseUrl);
    if (!href) continue;

    if (!best || priority > best.priority) {
      best = { href, priority };
    }
  }

  return best?.href ?? resolveDefaultFaviconUrl(baseUrl);
};

const fetchWithTimeout = async (
  input: string,
  timeoutMs: number,
  init?: RequestInit,
): Promise<Response> => {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  try {
    return await fetch(input, {
      ...init,
      headers: {
        "User-Agent": METADATA_USER_AGENT,
        ...(init?.headers ?? {}),
      },
      redirect: init?.redirect ?? "follow",
      signal: controller.signal,
    });
  } finally {
    clearTimeout(timer);
  }
};

const resolveImageContentType = (value: string, response: Response): string => {
  const headerValue = response.headers
    .get("content-type")
    ?.split(";")[0]
    ?.trim()
    ?.toLowerCase();
  if (headerValue?.startsWith("image/")) {
    return headerValue;
  }

  try {
    const { pathname } = new URL(value);
    const normalizedPath = pathname.toLowerCase();
    if (normalizedPath.endsWith(".svg")) return "image/svg+xml";
    if (normalizedPath.endsWith(".png")) return "image/png";
    if (normalizedPath.endsWith(".jpg") || normalizedPath.endsWith(".jpeg")) {
      return "image/jpeg";
    }
    if (normalizedPath.endsWith(".gif")) return "image/gif";
    if (normalizedPath.endsWith(".webp")) return "image/webp";
    if (normalizedPath.endsWith(".ico")) return "image/x-icon";
  } catch {
    // ignore
  }

  return "";
};

const fetchFaviconAsDataUrl = async (
  faviconUrl: string,
  timeoutMs: number,
): Promise<string> => {
  const normalizedUrl = normalizeHttpUrl(faviconUrl);
  if (!normalizedUrl) return "";

  try {
    const response = await fetchWithTimeout(normalizedUrl, timeoutMs, {
      headers: {
        Accept: "image/*,*/*;q=0.8",
      },
    });
    if (!response.ok) return "";

    const contentType = resolveImageContentType(normalizedUrl, response);
    if (!contentType) return "";

    const declaredLength = Number.parseInt(
      response.headers.get("content-length") ?? "",
      10,
    );
    if (Number.isFinite(declaredLength) && declaredLength > MAX_FAVICON_BYTES) {
      return "";
    }

    const bytes = Buffer.from(await response.arrayBuffer());
    if (bytes.byteLength === 0 || bytes.byteLength > MAX_FAVICON_BYTES) {
      return "";
    }

    return `data:${contentType};base64,${bytes.toString("base64")}`;
  } catch {
    return "";
  }
};

export const fetchUrlMetadata = async (
  inputUrl: string,
  timeoutMs: number = DEFAULT_REQUEST_TIMEOUT_MS,
): Promise<UrlMetadataResult> => {
  const normalizedUrl = normalizeHttpUrl(inputUrl);
  const fallbackData: UrlMetadata = {
    title: "",
    favicon: normalizedUrl ? resolveDefaultFaviconUrl(normalizedUrl) : "",
    finalUrl: normalizedUrl,
  };

  if (!normalizedUrl) {
    return {
      ok: false,
      data: fallbackData,
      error: "Only http/https targets are supported",
    };
  }

  try {
    const response = await fetchWithTimeout(normalizedUrl, timeoutMs, {
      headers: {
        Accept: "text/html,application/xhtml+xml,*/*;q=0.8",
      },
    });
    if (!response.ok) {
      return {
        ok: false,
        data: fallbackData,
        error: `Upstream responded with ${response.status}`,
      };
    }

    const finalUrl = response.url || normalizedUrl;
    const html = (await response.text()).slice(0, MAX_HTML_LENGTH);
    const faviconUrl = extractFaviconFromHtml(html, finalUrl);
    const favicon = await fetchFaviconAsDataUrl(faviconUrl, timeoutMs);

    return {
      ok: true,
      data: {
        title: extractTitleFromHtml(html),
        favicon,
        finalUrl,
      },
    };
  } catch (error) {
    return {
      ok: false,
      data: fallbackData,
      error:
        error instanceof Error ? error.message : "Failed to fetch metadata",
    };
  }
};
