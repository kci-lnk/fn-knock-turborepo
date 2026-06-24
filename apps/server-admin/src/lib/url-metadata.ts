import { Buffer } from "node:buffer";
import { fetchWithRelaxedTls } from "./relaxed-tls-fetch";

const DEFAULT_REQUEST_TIMEOUT_MS = 5000;
const MAX_HTML_LENGTH = 256 * 1024;
const MAX_MANIFEST_LENGTH = 64 * 1024;
const MAX_MANIFEST_ICONS_TO_TRY = 4;
const MAX_HTML_FAVICON_CANDIDATES_TO_TRY = 12;
const MAX_FAVICON_FETCH_ATTEMPTS = 8;
const FALLBACK_FAVICON_FETCH_RESERVE = 3;
const HEURISTIC_FAVICON_MIN_PRIORITY = 350;
const STRONG_HEURISTIC_FAVICON_MIN_PRIORITY = 520;
const MAX_FAVICON_BYTES = 128 * 1024;
const METADATA_USER_AGENT = "fn-knock-server-admin/1.0";
const MAX_METADATA_REDIRECTS = 20;
const REDIRECT_STATUSES = new Set([301, 302, 303, 307, 308]);
const ONE_PANEL_TITLE = "1Panel";
const ONE_PANEL_LOADING_TITLE = "loading...";
const ONE_PANEL_FAVICON_PATH = "/public/favicon.png";
const OPENWRT_LUCI_PATH = "/cgi-bin/luci/";
const OPENWRT_LUCI_LOGIN_REQUIRED_HEADER = "x-luci-login-required";
const OPENWRT_LUCI_TITLE_PATTERN = /(?:^|[^a-z0-9])luci(?:$|[^a-z0-9])/i;
const FALLBACK_FAVICON_PATHS = [
  "/favicon.ico",
  "/img/favicon.ico",
  ONE_PANEL_FAVICON_PATH,
];
const FAVICON_CANDIDATE_ATTRIBUTE_NAMES = [
  "href",
  "src",
  "content",
  "icon",
  "data-href",
  "data-src",
  "data-original",
  "data-icon",
  "data-favicon",
];
const HTML_IMAGE_RESOURCE_PATH_REGEX =
  /((?:(?:https?:)?\/\/|\/|\.{1,2}\/|[A-Za-z0-9_.-]+\/)[^\s"'<>\\)]*?\.(?:ico|png|svg|jpe?g|gif|webp)(?:[?#][^\s"'<>\\)]*)?)/gi;

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

type FaviconCandidate = {
  href: string;
  priority: number;
  index: number;
};

type FaviconCandidateContext = {
  tagName?: string;
  attributeName?: string;
  attributes?: Record<string, string>;
  surroundingText?: string;
  sourcePriority?: number;
  minPriority?: number;
  force?: boolean;
};

type FaviconFetchBudget = {
  remaining: number;
  seen: Set<string>;
};

type MetadataHtmlDocument = {
  html: string;
  finalUrl: string;
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

const resolveOriginPathUrl = (value: string, pathname: string): string => {
  try {
    const parsed = new URL(value);
    return `${parsed.origin}${pathname}`;
  } catch {
    return "";
  }
};

const resolveDefaultFaviconUrl = (value: string): string =>
  resolveOriginPathUrl(value, "/favicon.ico");

const resolveFallbackFaviconUrls = (value: string): string[] => {
  try {
    const parsed = new URL(value);
    return FALLBACK_FAVICON_PATHS.map(
      (pathname) => `${parsed.origin}${pathname}`,
    );
  } catch {
    return [];
  }
};

export const extractTitleFromHtml = (html: string): string => {
  const match = html.match(/<title\b[^>]*>([\s\S]*?)<\/title>/i);
  return collapseWhitespace(decodeHtmlEntities(match?.[1] ?? ""));
};

const isOpenWrtLuciUrl = (value: string): boolean => {
  try {
    const pathname = new URL(value).pathname.toLowerCase();
    return (
      pathname === "/cgi-bin/luci" || pathname.startsWith(OPENWRT_LUCI_PATH)
    );
  } catch {
    return false;
  }
};

const isSameOriginUrl = (value: string, baseUrl: string): boolean => {
  try {
    return new URL(value).origin === new URL(baseUrl).origin;
  } catch {
    return false;
  }
};

const stripRefreshUrlQuotes = (value: string): string =>
  value.trim().replace(/^["']|["']$/g, "");

const extractOpenWrtLuciUrlFromHtml = (
  html: string,
  baseUrl: string,
): string => {
  const metaTags = html.match(/<meta\b[^>]*>/gi) ?? [];

  for (const tag of metaTags) {
    const attributes = parseHtmlAttributes(tag);
    if (attributes["http-equiv"]?.trim().toLowerCase() !== "refresh") {
      continue;
    }

    const refreshUrl = decodeHtmlEntities(attributes.content ?? "").match(
      /\burl\s*=\s*([^;]+)/i,
    )?.[1];
    if (!refreshUrl) continue;

    const resolved = normalizeManifestUrl(
      stripRefreshUrlQuotes(refreshUrl),
      baseUrl,
    );
    if (
      resolved &&
      isOpenWrtLuciUrl(resolved) &&
      isSameOriginUrl(resolved, baseUrl)
    ) {
      return resolved;
    }
  }

  const linkTags = html.match(/<a\b[^>]*>/gi) ?? [];
  for (const tag of linkTags) {
    const attributes = parseHtmlAttributes(tag);
    const resolved = normalizeManifestUrl(attributes.href ?? "", baseUrl);
    if (
      resolved &&
      isOpenWrtLuciUrl(resolved) &&
      isSameOriginUrl(resolved, baseUrl)
    ) {
      return resolved;
    }
  }

  try {
    return new URL(OPENWRT_LUCI_PATH, baseUrl).toString();
  } catch {
    return "";
  }
};

const hasOpenWrtLuciEntrypointHtml = (html: string): boolean => {
  const normalized = html.toLowerCase();
  return (
    normalized.includes("cgi-bin/luci") &&
    (normalized.includes("luci - lua configuration interface") ||
      normalized.includes('http-equiv="refresh"') ||
      normalized.includes("http-equiv='refresh'") ||
      normalized.includes("http-equiv=refresh"))
  );
};

const hasOpenWrtLuciDocumentHtml = (html: string): boolean => {
  const title = extractTitleFromHtml(html).toLowerCase();
  const normalized = html.toLowerCase();
  return (
    OPENWRT_LUCI_TITLE_PATTERN.test(title) &&
    (normalized.includes("/luci-static/") ||
      normalized.includes("application-name") ||
      normalized.includes("apple-mobile-web-app-title"))
  );
};

const isOpenWrtLuciLoginRequiredResponse = (response: Response): boolean =>
  response.status === 403 &&
  response.headers
    .get(OPENWRT_LUCI_LOGIN_REQUIRED_HEADER)
    ?.trim()
    .toLowerCase() === "yes";

const isOnePanelLoadingTitle = (value: string): boolean =>
  value.trim().toLowerCase() === ONE_PANEL_LOADING_TITLE;

const extractHtmlBaseUrl = (html: string, baseUrl: string): string => {
  const baseTags = html.match(/<base\b[^>]*>/gi) ?? [];

  for (const tag of baseTags) {
    const attributes = parseHtmlAttributes(tag);
    const href = normalizeManifestUrl(attributes.href ?? "", baseUrl);
    if (href) return href;
  }

  return baseUrl;
};

export const extractFaviconFromHtml = (
  html: string,
  baseUrl: string,
): string => {
  const htmlBaseUrl = extractHtmlBaseUrl(html, baseUrl);
  return (
    extractExplicitFaviconUrlsFromHtml(html, htmlBaseUrl)[0] ||
    extractHeuristicFaviconUrlsFromHtml(
      html,
      htmlBaseUrl,
      HEURISTIC_FAVICON_MIN_PRIORITY,
    )[0] ||
    resolveDefaultFaviconUrl(baseUrl)
  );
};

const sortFaviconCandidates = (candidates: FaviconCandidate[]): string[] => {
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
    .slice(0, MAX_HTML_FAVICON_CANDIDATES_TO_TRY);
};

const getHtmlTagName = (tag: string): string =>
  tag.match(/^<\s*([^\s/>]+)/)?.[1]?.toLowerCase() ?? "";

const getImageExtensionPriority = (extension: string): number => {
  if (extension === "ico") return 80;
  if (extension === "png") return 60;
  if (extension === "svg") return 50;
  if (extension === "webp") return 40;
  if (extension === "jpg" || extension === "jpeg") return 30;
  if (extension === "gif") return 20;
  return 0;
};

const getFaviconPathPriority = (value: string): number => {
  if (/^data:image\//i.test(value)) return 0;

  try {
    const parsed = new URL(value);
    const pathname = parsed.pathname.toLowerCase();
    const fileName = pathname.split("/").pop() ?? "";
    const extension = fileName.match(/\.([a-z0-9]+)$/)?.[1] ?? "";

    let priority = 0;
    if (fileName === "favicon.ico") {
      priority = 700;
    } else if (/^favicon(?:[-_.]|$)/.test(fileName)) {
      priority = 650;
    } else if (/^apple-touch-icon/.test(fileName)) {
      priority = 600;
    } else if (/^android-chrome/.test(fileName)) {
      priority = 560;
    } else if (/^mstile/.test(fileName)) {
      priority = 520;
    } else if (fileName.includes("favicon")) {
      priority = 500;
    } else if (pathname.includes("/favicon")) {
      priority = 450;
    } else if (
      /(?:^|[-_.])(?:app-?icon|site-?icon|touch-?icon|icon)(?:[-_.]|\.)/.test(
        fileName,
      )
    ) {
      priority = 220;
    } else if (extension === "ico") {
      priority = 180;
    } else if (/(?:^|[-_.])logo(?:[-_.]|\.)/.test(fileName)) {
      priority = 80;
    } else {
      return 0;
    }

    priority += getImageExtensionPriority(extension);
    if (pathname.includes("/img/")) priority += 20;
    if (pathname.includes("/icons/") || pathname.includes("/icon/")) {
      priority += 15;
    }
    if (pathname.split("/").length <= 3) priority += 10;
    return priority;
  } catch {
    return 0;
  }
};

const getFaviconTypePriority = (value: string): number => {
  const normalized = value.split(";")[0]?.trim().toLowerCase() ?? "";
  if (
    normalized === "image/x-icon" ||
    normalized === "image/vnd.microsoft.icon" ||
    normalized === "application/x-icon" ||
    normalized === "application/vnd.microsoft.icon"
  ) {
    return 850;
  }
  if (normalized === "image/svg+xml") return 260;
  if (normalized.startsWith("image/")) return 160;
  return 0;
};

const getAttributeHintPriority = (
  attributeName: string,
  attributes: Record<string, string> | undefined,
): number => {
  let priority = 0;
  const normalizedAttributeName = attributeName.toLowerCase();
  if (normalizedAttributeName.includes("favicon")) priority += 450;
  else if (normalizedAttributeName.includes("icon")) priority += 280;
  else if (normalizedAttributeName === "href") priority += 60;
  else if (normalizedAttributeName === "src") priority += 40;
  else if (normalizedAttributeName === "content") priority += 30;

  for (const value of [
    attributes?.name,
    attributes?.property,
    attributes?.itemprop,
    attributes?.id,
    attributes?.class,
  ]) {
    const normalizedValue = value?.trim().toLowerCase() ?? "";
    if (!normalizedValue) continue;
    if (
      normalizedValue.includes("favicon") ||
      normalizedValue.includes("shortcut icon")
    ) {
      priority += 520;
    } else if (normalizedValue.includes("apple-touch-icon")) {
      priority += 480;
    } else if (
      normalizedValue.includes("msapplication-tileimage") ||
      normalizedValue.includes("tileimage")
    ) {
      priority += 440;
    } else if (/\bicon\b/.test(normalizedValue)) {
      priority += 260;
    }
  }

  return priority;
};

const getTagPriority = (tagName: string): number => {
  if (tagName === "link") return 120;
  if (tagName === "meta") return 60;
  if (tagName === "img") return 20;
  return 0;
};

const getHtmlIconSizePriority = (sizes: string | undefined): number => {
  if (!sizes) return 0;

  let best = 0;
  for (const token of sizes.trim().toLowerCase().split(/\s+/)) {
    if (token === "any") {
      best = Math.max(best, 1024);
      continue;
    }

    const match = token.match(/^(\d+)x(\d+)$/);
    if (!match) continue;

    const width = Number.parseInt(match[1] ?? "", 10);
    const height = Number.parseInt(match[2] ?? "", 10);
    if (!Number.isFinite(width) || !Number.isFinite(height)) continue;

    best = Math.max(best, Math.min(width, height));
  }

  if (best >= 192) return 160;
  if (best >= 64) return 120;
  if (best >= 32) return 80;
  if (best > 0) return 30;
  return 0;
};

const getSurroundingFaviconPriority = (value: string | undefined): number => {
  const normalized = value?.toLowerCase() ?? "";
  if (!normalized) return 0;
  if (normalized.includes("favicon")) return 520;
  if (normalized.includes("shortcut icon")) return 500;
  if (normalized.includes("apple-touch-icon")) return 480;
  if (
    normalized.includes("msapplication-tileimage") ||
    normalized.includes("tileimage")
  ) {
    return 440;
  }
  if (
    /(?:fav[-_ ]?icon|icon(?:url|uri|href|src|path)|appicon|siteicon)/.test(
      normalized,
    )
  ) {
    return 320;
  }
  if (/\bicon\b/.test(normalized)) return 140;
  return 0;
};

const normalizeFaviconCandidateUrl = (value: string, baseUrl: string): string =>
  normalizeFaviconUrl(value.replace(/\\\//g, "/"), baseUrl);

const createFaviconCandidate = (
  rawValue: string,
  baseUrl: string,
  index: number,
  context: FaviconCandidateContext,
): FaviconCandidate | null => {
  const href = normalizeFaviconCandidateUrl(rawValue, baseUrl);
  if (!href) return null;

  const attributes = context.attributes;
  const relPriority = getFaviconPriority(attributes?.rel ?? "");
  const pathPriority = getFaviconPathPriority(href);
  const typePriority = getFaviconTypePriority(attributes?.type ?? "");
  const attributePriority = getAttributeHintPriority(
    context.attributeName ?? "",
    attributes,
  );
  const surroundingPriority = getSurroundingFaviconPriority(
    context.surroundingText,
  );
  const priority =
    relPriority * 1000 +
    pathPriority +
    typePriority +
    attributePriority +
    surroundingPriority +
    getTagPriority(context.tagName ?? "") +
    getHtmlIconSizePriority(attributes?.sizes) +
    (context.sourcePriority ?? 0);

  if (!context.force && priority < (context.minPriority ?? 350)) {
    return null;
  }

  return { href, priority, index };
};

const extractExplicitFaviconUrlsFromHtml = (
  html: string,
  baseUrl: string,
): string[] => {
  const linkTags = html.match(/<link\b[^>]*>/gi) ?? [];
  const candidates: FaviconCandidate[] = [];

  for (const [index, tag] of linkTags.entries()) {
    const attributes = parseHtmlAttributes(tag);
    const priority = getFaviconPriority(attributes.rel ?? "");
    if (priority <= 0) continue;

    const candidate = createFaviconCandidate(
      attributes.href ?? "",
      baseUrl,
      index,
      {
        tagName: getHtmlTagName(tag),
        attributeName: "href",
        attributes,
        force: true,
      },
    );
    if (candidate) candidates.push(candidate);
  }

  return sortFaviconCandidates(candidates);
};

const extractHeuristicFaviconUrlsFromHtml = (
  html: string,
  baseUrl: string,
  minPriority = 350,
): string[] => {
  const candidates: FaviconCandidate[] = [];
  let index = 0;
  const tags = html.match(/<(?:link|meta|img|source)\b[^>]*>/gi) ?? [];

  for (const tag of tags) {
    const tagName = getHtmlTagName(tag);
    const attributes = parseHtmlAttributes(tag);
    for (const attributeName of FAVICON_CANDIDATE_ATTRIBUTE_NAMES) {
      const rawValue = attributes[attributeName];
      if (!rawValue) continue;

      const candidate = createFaviconCandidate(rawValue, baseUrl, index, {
        tagName,
        attributeName,
        attributes,
        minPriority,
      });
      if (candidate) candidates.push(candidate);
      index += 1;
    }
  }

  for (const match of html.matchAll(HTML_IMAGE_RESOURCE_PATH_REGEX)) {
    const rawValue = match[1];
    if (!rawValue) continue;
    const matchIndex = match.index ?? 0;
    const surroundingText = html.slice(
      Math.max(0, matchIndex - 80),
      matchIndex + rawValue.length + 80,
    );

    const candidate = createFaviconCandidate(rawValue, baseUrl, index, {
      surroundingText,
      minPriority,
    });
    if (candidate) candidates.push(candidate);
    index += 1;
  }

  return sortFaviconCandidates(candidates);
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

const fetchOpenWrtLuciDocument = async (
  document: MetadataHtmlDocument,
  timeoutMs: number,
  basicAuthContext: BasicAuthRequestContext | null,
): Promise<MetadataHtmlDocument | null> => {
  if (
    isOpenWrtLuciUrl(document.finalUrl) ||
    hasOpenWrtLuciDocumentHtml(document.html)
  ) {
    return document;
  }
  if (!hasOpenWrtLuciEntrypointHtml(document.html)) {
    return null;
  }

  const luciUrl = extractOpenWrtLuciUrlFromHtml(
    document.html,
    document.finalUrl,
  );
  if (!luciUrl) return null;

  try {
    const response = await fetchWithTimeout(
      luciUrl,
      timeoutMs,
      {
        headers: {
          Accept: "text/html,application/xhtml+xml,*/*;q=0.8",
        },
      },
      basicAuthContext,
    );
    const isLuciLoginRequired = isOpenWrtLuciLoginRequiredResponse(response);
    if (!response.ok && !isLuciLoginRequired) {
      return null;
    }

    const html = (await response.text()).slice(0, MAX_HTML_LENGTH);
    if (!hasOpenWrtLuciDocumentHtml(html) && !isLuciLoginRequired) {
      return null;
    }

    return {
      html,
      finalUrl: response.url || luciUrl,
    };
  } catch {
    return null;
  }
};

const resolveImageContentType = (value: string, response: Response): string => {
  const headerValue = response.headers
    .get("content-type")
    ?.split(";")[0]
    ?.trim()
    ?.toLowerCase();
  if (
    headerValue === "application/ico" ||
    headerValue === "application/x-ico" ||
    headerValue === "application/x-icon" ||
    headerValue === "application/vnd.microsoft.icon"
  ) {
    return "image/x-icon";
  }
  if (headerValue?.startsWith("image/")) {
    return headerValue;
  }
  if (
    headerValue &&
    headerValue !== "application/octet-stream" &&
    headerValue !== "binary/octet-stream"
  ) {
    return "";
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
  budget: FaviconFetchBudget = {
    remaining: MAX_FAVICON_FETCH_ATTEMPTS,
    seen: new Set<string>(),
  },
  reserveAttempts = 0,
): Promise<string> => {
  for (const faviconUrl of faviconUrls) {
    const normalized = faviconUrl.trim();
    if (!normalized || budget.seen.has(normalized)) continue;

    const isInlineImage = /^data:image\//i.test(normalized);
    if (!isInlineImage) {
      if (budget.remaining <= reserveAttempts) break;
      budget.remaining -= 1;
    }

    budget.seen.add(normalized);
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
    const isOpenWrtLuciLoginRequired =
      isOpenWrtLuciLoginRequiredResponse(response);
    if (!response.ok && !isOpenWrtLuciLoginRequired) {
      return {
        ok: false,
        data: fallbackData,
        error: `Upstream responded with ${response.status}`,
      };
    }

    const initialDocument: MetadataHtmlDocument = {
      finalUrl: response.url || normalizedUrl,
      html: (await response.text()).slice(0, MAX_HTML_LENGTH),
    };
    const metadataDocument =
      (await fetchOpenWrtLuciDocument(
        initialDocument,
        timeoutMs,
        basicAuthContext,
      )) ?? initialDocument;
    const { finalUrl, html } = metadataDocument;
    const title = extractTitleFromHtml(html);
    const onePanelFavicon = isOnePanelLoadingTitle(title)
      ? await fetchFaviconAsDataUrl(
          resolveOriginPathUrl(finalUrl, ONE_PANEL_FAVICON_PATH),
          timeoutMs,
          basicAuthContext,
        )
      : "";
    if (onePanelFavicon) {
      return {
        ok: true,
        data: {
          title: ONE_PANEL_TITLE,
          favicon: onePanelFavicon,
          finalUrl,
        },
      };
    }

    const htmlBaseUrl = extractHtmlBaseUrl(html, finalUrl);
    const explicitFaviconUrls = extractExplicitFaviconUrlsFromHtml(
      html,
      htmlBaseUrl,
    );
    const strongHeuristicFaviconUrls = extractHeuristicFaviconUrlsFromHtml(
      html,
      htmlBaseUrl,
      STRONG_HEURISTIC_FAVICON_MIN_PRIORITY,
    );
    const weakHeuristicFaviconUrls = extractHeuristicFaviconUrlsFromHtml(
      html,
      htmlBaseUrl,
      HEURISTIC_FAVICON_MIN_PRIORITY,
    );
    const manifestUrl = extractManifestFromHtml(html, htmlBaseUrl);
    const faviconFetchBudget: FaviconFetchBudget = {
      remaining: MAX_FAVICON_FETCH_ATTEMPTS,
      seen: new Set<string>(),
    };
    let favicon = await fetchFirstFaviconAsDataUrl(
      explicitFaviconUrls,
      timeoutMs,
      basicAuthContext,
      faviconFetchBudget,
      FALLBACK_FAVICON_FETCH_RESERVE,
    );
    if (!favicon && manifestUrl) {
      favicon = await fetchFirstFaviconAsDataUrl(
        await fetchManifestIconUrls(manifestUrl, timeoutMs, basicAuthContext),
        timeoutMs,
        basicAuthContext,
        faviconFetchBudget,
        FALLBACK_FAVICON_FETCH_RESERVE,
      );
    }
    if (!favicon) {
      favicon = await fetchFirstFaviconAsDataUrl(
        strongHeuristicFaviconUrls,
        timeoutMs,
        basicAuthContext,
        faviconFetchBudget,
        FALLBACK_FAVICON_FETCH_RESERVE,
      );
    }
    if (!favicon) {
      favicon = await fetchFirstFaviconAsDataUrl(
        resolveFallbackFaviconUrls(finalUrl),
        timeoutMs,
        basicAuthContext,
        faviconFetchBudget,
      );
    }
    if (!favicon) {
      favicon = await fetchFirstFaviconAsDataUrl(
        weakHeuristicFaviconUrls,
        timeoutMs,
        basicAuthContext,
        faviconFetchBudget,
      );
    }

    return {
      ok: true,
      data: {
        title,
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
