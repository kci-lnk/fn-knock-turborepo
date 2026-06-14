import {
  DEFAULT_LOCALE,
  interpolateMessage,
  normalizeLocale,
  readMessagePath,
  type LocaleCode,
  type MessageParams,
} from "./core";

export type BrowserI18nScope = "admin" | "auth";
export type ScopedLocaleMessages = Record<string, unknown>;
export type LocaleMessagesModule = {
  default: ScopedLocaleMessages;
};
export type LocaleLoaderMap = Record<
  LocaleCode,
  () => Promise<LocaleMessagesModule>
>;

const scopedLoaders = new Map<BrowserI18nScope, LocaleLoaderMap>();
const loadedMessages = new Map<string, ScopedLocaleMessages>();
const pendingLoads = new Map<string, Promise<ScopedLocaleMessages>>();

let activeScope: BrowserI18nScope | null = null;
let activeLocale: LocaleCode = DEFAULT_LOCALE;

const cacheKey = (scope: BrowserI18nScope, locale: LocaleCode) =>
  `${scope}:${locale}`;

export const registerScopedLocaleLoaders = (
  scope: BrowserI18nScope,
  loaders: LocaleLoaderMap,
) => {
  scopedLoaders.set(scope, loaders);
};

const getScopedLocaleLoaders = (scope: BrowserI18nScope): LocaleLoaderMap => {
  const loaders = scopedLoaders.get(scope);
  if (!loaders) {
    throw new Error(`[i18n] no locale loaders registered for scope "${scope}"`);
  }
  return loaders;
};

export const setActiveBrowserLocale = (
  scope: BrowserI18nScope,
  locale: LocaleCode,
) => {
  activeScope = scope;
  activeLocale = locale;
};

export const getActiveBrowserScope = (): BrowserI18nScope =>
  activeScope ?? "admin";

export const getActiveBrowserLocale = (): LocaleCode => activeLocale;

export const getLoadedScopedLocaleMessages = (
  scope: BrowserI18nScope,
  locale: LocaleCode,
): ScopedLocaleMessages | null =>
  loadedMessages.get(cacheKey(scope, locale)) ?? null;

export const loadScopedLocaleMessages = async (
  scope: BrowserI18nScope,
  locale: LocaleCode,
): Promise<ScopedLocaleMessages> => {
  const key = cacheKey(scope, locale);
  const loaded = loadedMessages.get(key);
  if (loaded) return loaded;

  const pending = pendingLoads.get(key);
  if (pending) return pending;

  const load = getScopedLocaleLoaders(scope)
    [locale]()
    .then((module) => {
      loadedMessages.set(key, module.default);
      return module.default;
    })
    .finally(() => {
      pendingLoads.delete(key);
    });

  pendingLoads.set(key, load);
  return load;
};

export const ensureScopedLocaleReady = async (
  scope: BrowserI18nScope,
  locale: LocaleCode,
): Promise<void> => {
  await Promise.all(
    locale === DEFAULT_LOCALE
      ? [loadScopedLocaleMessages(scope, locale)]
      : [
          loadScopedLocaleMessages(scope, locale),
          loadScopedLocaleMessages(scope, DEFAULT_LOCALE),
        ],
  );
};

export const getScopedLocaleMessages = (
  scope: BrowserI18nScope,
  locale: LocaleCode,
): Partial<Record<LocaleCode, ScopedLocaleMessages>> => {
  const entries = [DEFAULT_LOCALE, locale]
    .map((code) => [code, getLoadedScopedLocaleMessages(scope, code)] as const)
    .filter((entry): entry is readonly [LocaleCode, ScopedLocaleMessages] =>
      Boolean(entry[1]),
    );

  return Object.fromEntries(entries) as Partial<
    Record<LocaleCode, ScopedLocaleMessages>
  >;
};

export const translateLoadedBrowserMessage = (
  key: string,
  params?: MessageParams,
  localeValue?: string | null,
): string => {
  const scope = getActiveBrowserScope();
  const locale = normalizeLocale(localeValue) ?? getActiveBrowserLocale();
  const value =
    readMessagePath(getLoadedScopedLocaleMessages(scope, locale), key) ??
    readMessagePath(getLoadedScopedLocaleMessages(scope, DEFAULT_LOCALE), key);
  if (typeof value !== "string") return key;
  return interpolateMessage(value, params);
};
