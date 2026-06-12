import { Buffer } from "node:buffer";
import { fetchWithRelaxedTls } from "./relaxed-tls-fetch";

const DEFAULT_REQUEST_TIMEOUT_MS = 5000;
const MAX_HTML_LENGTH = 256 * 1024;
const MAX_MANIFEST_LENGTH = 64 * 1024;
const MAX_MANIFEST_ICONS_TO_TRY = 4;
const MAX_FAVICON_BYTES = 128 * 1024;
const METADATA_USER_AGENT = "fn-knock-server-admin/1.0";
const MAX_METADATA_REDIRECTS = 20;
const REDIRECT_STATUSES = new Set([301, 302, 303, 307, 308]);

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

export interface UrlMetadataBasicAuth {
  enabled?: boolean;
  username?: string;
  password?: string;
}

export interface FetchUrlMetadataOptions {
  timeoutMs?: number;
  basicAuth?: UrlMetadataBasicAuth | null;
}

type BasicAuthRequestContext = {
  origin: string;
  authorization: string;
};

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
  const trimmed = decodeHtmlEntities(value).trim();
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

const normalizeManifestUrl = (value: string, baseUrl: string): string => {
  const trimmed = decodeHtmlEntities(value).trim();
  if (!trimmed) return "";

  try {
    const resolved = new URL(trimmed, baseUrl);
    if (resolved.protocol !== "http:" && resolved.protocol !== "https:") {
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
  return (
    extractExplicitFaviconFromHtml(html, baseUrl) ||
    resolveDefaultFaviconUrl(baseUrl)
  );
};

const extractExplicitFaviconFromHtml = (
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

  return best?.href ?? "";
};

const extractManifestFromHtml = (html: string, baseUrl: string): string => {
  const linkTags = html.match(/<link\b[^>]*>/gi) ?? [];

  for (const tag of linkTags) {
    const attributes = parseHtmlAttributes(tag);
    const relTokens = (attributes.rel ?? "")
      .trim()
      .toLowerCase()
      .split(/\s+/)
      .filter(Boolean);
    if (!relTokens.includes("manifest")) continue;

    const href = normalizeManifestUrl(attributes.href ?? "", baseUrl);
    if (href) return href;
  }

  return "";
};

const normalizeRequestTimeout = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value) && value > 0
    ? value
    : DEFAULT_REQUEST_TIMEOUT_MS;

const normalizeFetchUrlMetadataOptions = (
  options?: number | FetchUrlMetadataOptions,
): Required<Pick<FetchUrlMetadataOptions, "timeoutMs">> &
  Pick<FetchUrlMetadataOptions, "basicAuth"> => {
  if (typeof options === "number") {
    return {
      timeoutMs: normalizeRequestTimeout(options),
      basicAuth: null,
    };
  }

  return {
    timeoutMs: normalizeRequestTimeout(options?.timeoutMs),
    basicAuth: options?.basicAuth ?? null,
  };
};

const createBasicAuthRequestContext = (
  value: UrlMetadataBasicAuth | null | undefined,
  targetUrl: string,
): BasicAuthRequestContext | null => {
  const username =
    typeof value?.username === "string" ? value.username.trim() : "";
  const password = typeof value?.password === "string" ? value.password : "";
  if (
    value?.enabled !== true ||
    !username ||
    !password ||
    username.includes(":")
  ) {
    return null;
  }

  try {
    return {
      origin: new URL(targetUrl).origin,
      authorization: `Basic ${Buffer.from(
        `${username}:${password}`,
        "utf8",
      ).toString("base64")}`,
    };
  } catch {
    return null;
  }
};

const hasSameOrigin = (value: string, origin: string): boolean => {
  try {
    return new URL(value).origin === origin;
  } catch {
    return false;
  }
};

const createMetadataRequestHeaders = (
  input: string,
  initHeaders: HeadersInit | undefined,
  basicAuthContext: BasicAuthRequestContext | null,
): Headers => {
  const headers = new Headers(initHeaders);
  headers.set("User-Agent", METADATA_USER_AGENT);
  if (
    basicAuthContext &&
    hasSameOrigin(input, basicAuthContext.origin) &&
    !headers.has("Authorization")
  ) {
    headers.set("Authorization", basicAuthContext.authorization);
  }
  return headers;
};

const fetchWithTimeout = async (
  input: string,
  timeoutMs: number,
  init?: RequestInit,
  basicAuthContext: BasicAuthRequestContext | null = null,
): Promise<Response> => {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  try {
    if (!basicAuthContext) {
      return await fetchWithRelaxedTls(input, {
        ...init,
        headers: createMetadataRequestHeaders(input, init?.headers, null),
        redirect: init?.redirect ?? "follow",
        signal: controller.signal,
      });
    }

    let requestUrl = input;
    for (let redirectCount = 0; ; redirectCount += 1) {
      const response = await fetchWithRelaxedTls(requestUrl, {
        ...init,
        headers: createMetadataRequestHeaders(
          requestUrl,
          init?.headers,
          basicAuthContext,
        ),
        redirect: "manual",
        signal: controller.signal,
      });

      const location = response.headers.get("location");
      if (!REDIRECT_STATUSES.has(response.status) || !location) {
        return response;
      }

      if (init?.redirect === "error") {
        throw new TypeError(`Redirect received from ${requestUrl}`);
      }
      if (init?.redirect === "manual") {
        return response;
      }
      if (redirectCount >= MAX_METADATA_REDIRECTS) {
        throw new TypeError("Maximum redirect reached");
      }

      requestUrl = new URL(location, requestUrl).toString();
    }
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
  basicAuthContext: BasicAuthRequestContext | null,
): Promise<string> => {
  const trimmedUrl = faviconUrl.trim();
  if (/^data:image\//i.test(trimmedUrl)) {
    return Buffer.byteLength(trimmedUrl, "utf8") <= MAX_FAVICON_BYTES * 2
      ? trimmedUrl
      : "";
  }

  const normalizedUrl = normalizeHttpUrl(faviconUrl);
  if (!normalizedUrl) return "";

  try {
    const response = await fetchWithTimeout(
      normalizedUrl,
      timeoutMs,
      {
        headers: {
          Accept: "image/*,*/*;q=0.8",
        },
      },
      basicAuthContext,
    );
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

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const getManifestIconSizeScore = (sizes: unknown): number => {
  if (typeof sizes !== "string") return 0;

  let best = 0;
  for (const token of sizes.trim().toLowerCase().split(/\s+/)) {
    if (token === "any") {
      best = Math.max(best, 1024);
      continue;
    }

    const match = token.match(/^(\d+)x(\d+)$/);
    if (!match) continue;

    const [, widthText, heightText] = match;
    if (!widthText || !heightText) continue;

    const width = Number.parseInt(widthText, 10);
    const height = Number.parseInt(heightText, 10);
    if (!Number.isFinite(width) || !Number.isFinite(height)) continue;

    best = Math.max(best, Math.min(width, height));
  }

  return best;
};

const getManifestIconPriority = (icon: Record<string, unknown>): number => {
  const purposeTokens =
    typeof icon.purpose === "string"
      ? icon.purpose.trim().toLowerCase().split(/\s+/).filter(Boolean)
      : [];
  const type =
    typeof icon.type === "string"
      ? (icon.type.split(";")[0]?.trim().toLowerCase() ?? "")
      : "";

  let priority = getManifestIconSizeScore(icon.sizes);
  if (purposeTokens.length === 0 || purposeTokens.includes("any")) {
    priority += 2000;
  } else if (purposeTokens.includes("maskable")) {
    priority += 1000;
  }

  if (type === "image/png") priority += 80;
  else if (type === "image/svg+xml") priority += 70;
  else if (type === "image/webp") priority += 60;
  else if (type === "image/jpeg") priority += 50;
  else if (type === "image/x-icon" || type === "image/vnd.microsoft.icon") {
    priority += 40;
  }

  return priority;
};

const extractManifestIconUrls = (
  manifest: unknown,
  manifestUrl: string,
): string[] => {
  if (!isRecord(manifest) || !Array.isArray(manifest.icons)) {
    return [];
  }

  const candidates: Array<{ href: string; priority: number; index: number }> =
    [];

  for (const [index, rawIcon] of manifest.icons.entries()) {
    if (!isRecord(rawIcon) || typeof rawIcon.src !== "string") continue;

    const type =
      typeof rawIcon.type === "string"
        ? rawIcon.type.split(";")[0]?.trim().toLowerCase()
        : "";
    if (type && !type.startsWith("image/")) continue;

    const href = normalizeFaviconUrl(rawIcon.src, manifestUrl);
    if (!href) continue;

    candidates.push({
      href,
      priority: getManifestIconPriority(rawIcon),
      index,
    });
  }

  const seen = new Set<string>();
  return candidates
    .sort(
      (left, right) =>
        right.priority - left.priority || left.index - right.index,
    )
    .map((candidate) => candidate.href)
    .filter((href) => {
      if (seen.has(href)) return false;
      seen.add(href);
      return true;
    })
    .slice(0, MAX_MANIFEST_ICONS_TO_TRY);
};

const fetchManifestIconUrls = async (
  manifestUrl: string,
  timeoutMs: number,
  basicAuthContext: BasicAuthRequestContext | null,
): Promise<string[]> => {
  const normalizedUrl = normalizeHttpUrl(manifestUrl);
  if (!normalizedUrl) return [];

  try {
    const response = await fetchWithTimeout(
      normalizedUrl,
      timeoutMs,
      {
        headers: {
          Accept: "application/manifest+json,application/json,*/*;q=0.8",
        },
      },
      basicAuthContext,
    );
    if (!response.ok) return [];

    const declaredLength = Number.parseInt(
      response.headers.get("content-length") ?? "",
      10,
    );
    if (
      Number.isFinite(declaredLength) &&
      declaredLength > MAX_MANIFEST_LENGTH
    ) {
      return [];
    }

    const manifestText = (await response.text())
      .slice(0, MAX_MANIFEST_LENGTH)
      .replace(/^\uFEFF/u, "");
    const manifest = JSON.parse(manifestText) as unknown;

    return extractManifestIconUrls(manifest, response.url || normalizedUrl);
  } catch {
    return [];
  }
};

const fetchFirstFaviconAsDataUrl = async (
  faviconUrls: string[],
  timeoutMs: number,
  basicAuthContext: BasicAuthRequestContext | null,
): Promise<string> => {
  const seen = new Set<string>();

  for (const faviconUrl of faviconUrls) {
    const normalized = faviconUrl.trim();
    if (!normalized || seen.has(normalized)) continue;

    seen.add(normalized);
    const favicon = await fetchFaviconAsDataUrl(
      normalized,
      timeoutMs,
      basicAuthContext,
    );
    if (favicon) return favicon;
  }

  return "";
};

export const fetchUrlMetadata = async (
  inputUrl: string,
  options?: number | FetchUrlMetadataOptions,
): Promise<UrlMetadataResult> => {
  const { timeoutMs, basicAuth } = normalizeFetchUrlMetadataOptions(options);
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

  const basicAuthContext = createBasicAuthRequestContext(
    basicAuth,
    normalizedUrl,
  );

  try {
    const response = await fetchWithTimeout(
      normalizedUrl,
      timeoutMs,
      {
        headers: {
          Accept: "text/html,application/xhtml+xml,*/*;q=0.8",
        },
      },
      basicAuthContext,
    );
    if (!response.ok) {
      return {
        ok: false,
        data: fallbackData,
        error: `Upstream responded with ${response.status}`,
      };
    }

    const finalUrl = response.url || normalizedUrl;
    const html = (await response.text()).slice(0, MAX_HTML_LENGTH);
    const explicitFaviconUrl = extractExplicitFaviconFromHtml(html, finalUrl);
    const manifestUrl = extractManifestFromHtml(html, finalUrl);
    let favicon = explicitFaviconUrl
      ? await fetchFaviconAsDataUrl(
          explicitFaviconUrl,
          timeoutMs,
          basicAuthContext,
        )
      : "";
    if (!favicon && manifestUrl) {
      favicon = await fetchFirstFaviconAsDataUrl(
        await fetchManifestIconUrls(
          manifestUrl,
          timeoutMs,
          basicAuthContext,
        ),
        timeoutMs,
        basicAuthContext,
      );
    }
    if (!favicon) {
      favicon = await fetchFaviconAsDataUrl(
        resolveDefaultFaviconUrl(finalUrl),
        timeoutMs,
        basicAuthContext,
      );
    }

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
