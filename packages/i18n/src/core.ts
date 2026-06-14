export { LOCALE_DISPLAY_NAMES, LOCALE_OPTIONS } from "./locale-options";

export const SUPPORTED_LOCALES = ["zh-CN", "zh-Hant", "en"] as const;
export type LocaleCode = (typeof SUPPORTED_LOCALES)[number];

export interface LocaleConfig {
  default_locale: LocaleCode;
}

export const DEFAULT_LOCALE: LocaleCode = "zh-CN";
export const LOCALE_COOKIE_NAME = "fn_knock_locale";
export const LOCALE_STORAGE_KEY = "fn-knock:locale";
export const LOCALE_HEADER_NAME = "X-Fn-Knock-Locale";

const supportedLocaleSet = new Set<string>(SUPPORTED_LOCALES);

const localeAliases: Record<string, LocaleCode> = {
  zh: "zh-CN",
  "zh-cn": "zh-CN",
  "zh-hans": "zh-CN",
  "zh-hans-cn": "zh-CN",
  "zh-sg": "zh-CN",
  "zh-my": "zh-CN",
  "zh-tw": "zh-Hant",
  "zh-hk": "zh-Hant",
  "zh-mo": "zh-Hant",
  "zh-hant": "zh-Hant",
  "zh-hant-tw": "zh-Hant",
  en: "en",
  "en-us": "en",
  "en-gb": "en",
};

export const normalizeLocale = (
  value: string | null | undefined,
): LocaleCode | null => {
  const raw = String(value ?? "").trim();
  if (!raw) return null;
  if (supportedLocaleSet.has(raw)) return raw as LocaleCode;

  const lower = raw.replace(/_/g, "-").toLowerCase();
  if (localeAliases[lower]) return localeAliases[lower];
  if (lower.startsWith("en-")) return "en";
  if (lower.startsWith("zh-hant")) return "zh-Hant";
  if (lower.startsWith("zh-")) return "zh-CN";
  return null;
};

export const normalizeLocaleConfig = (
  value: Partial<LocaleConfig> | null | undefined,
): LocaleConfig => ({
  default_locale: normalizeLocale(value?.default_locale) ?? DEFAULT_LOCALE,
});

export const parseAcceptLanguage = (
  header: string | null | undefined,
): LocaleCode | null => {
  const raw = String(header ?? "").trim();
  if (!raw) return null;

  const candidates = raw
    .split(",")
    .map((part) => {
      const [rawTag, ...params] = part.trim().split(";");
      const tag = rawTag ?? "";
      const qParam = params.find((param) => param.trim().startsWith("q="));
      const q = qParam ? Number(qParam.trim().slice(2)) : 1;
      return {
        tag: tag.trim(),
        q: Number.isFinite(q) ? q : 1,
      };
    })
    .filter((candidate) => candidate.tag && candidate.q > 0)
    .sort((left, right) => right.q - left.q);

  for (const candidate of candidates) {
    const normalized = normalizeLocale(candidate.tag);
    if (normalized) return normalized;
  }

  return null;
};

const getCookieValue = (cookieHeader: string, name: string): string | null => {
  const prefix = `${name}=`;
  for (const part of cookieHeader.split(";")) {
    const item = part.trim();
    if (!item.startsWith(prefix)) continue;
    try {
      return decodeURIComponent(item.slice(prefix.length));
    } catch {
      return item.slice(prefix.length);
    }
  }
  return null;
};

export const getLocaleFromCookieHeader = (
  cookieHeader: string | null | undefined,
): LocaleCode | null =>
  cookieHeader
    ? normalizeLocale(getCookieValue(cookieHeader, LOCALE_COOKIE_NAME))
    : null;

export interface LocaleResolutionInput {
  explicitLocale?: string | null;
  cookieHeader?: string | null;
  storageLocale?: string | null;
  defaultLocale?: string | null;
  acceptLanguage?: string | null;
}

export const resolveLocale = (input: LocaleResolutionInput = {}): LocaleCode =>
  normalizeLocale(input.defaultLocale) ?? DEFAULT_LOCALE;

export type MessageParams = Record<
  string,
  string | number | boolean | null | undefined
>;

export const readMessagePath = (source: unknown, key: string): unknown => {
  let current = source;
  for (const part of key.split(".")) {
    if (current == null || typeof current !== "object" || !(part in current)) {
      return undefined;
    }
    current = (current as Record<string, unknown>)[part];
  }
  return current;
};

export const interpolateMessage = (
  message: string,
  params?: MessageParams,
): string =>
  message.replace(/\{([A-Za-z0-9_]+)\}/g, (match, name) => {
    const value = params?.[name];
    return value == null ? match : String(value);
  });
