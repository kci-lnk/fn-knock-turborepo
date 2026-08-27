import { createI18n } from "vue-i18n";
import {
  DEFAULT_LOCALE,
  LOCALE_COOKIE_NAME,
  LOCALE_STORAGE_KEY,
  type LocaleCode,
  type MessageParams,
  normalizeLocale,
  resolveLocale,
} from "./core";
import {
  ensureScopedLocaleReady,
  getActiveBrowserScope,
  getScopedLocaleMessages,
  loadScopedLocaleMessages,
  setActiveBrowserLocale,
  translateLoadedBrowserMessage,
  type BrowserI18nScope,
  type ScopedLocaleMessages,
} from "./browser-runtime";

export interface CreateFnKnockI18nOptions {
  scope?: BrowserI18nScope;
  defaultLocale?: string | null;
}

type VueLocaleMessage = Record<string, any>;
type VueLocaleMessages = Record<string, VueLocaleMessage>;

type LocaleMessageTarget = {
  locale: { value: string };
  setLocaleMessage: (locale: LocaleCode, message: VueLocaleMessage) => void;
};

const toVueLocaleMessage = (messages: ScopedLocaleMessages): VueLocaleMessage =>
  messages as VueLocaleMessage;

const toVueLocaleMessages = (
  messages: Partial<Record<LocaleCode, ScopedLocaleMessages>>,
): VueLocaleMessages => messages as VueLocaleMessages;

const getLocaleMessageTarget = (i18n: unknown): LocaleMessageTarget => {
  const maybeGlobal = (i18n as { global?: LocaleMessageTarget }).global;
  return maybeGlobal ?? (i18n as LocaleMessageTarget);
};

const browserLocaleInputs = () => {
  if (typeof document === "undefined") {
    return { cookieHeader: null, storageLocale: null };
  }
  let storageLocale: string | null = null;
  try {
    storageLocale =
      globalThis.localStorage?.getItem(LOCALE_STORAGE_KEY) ?? null;
  } catch {
    storageLocale = null;
  }
  return { cookieHeader: document.cookie, storageLocale };
};

export const hasBrowserLocalePreference = (): boolean => {
  const { cookieHeader, storageLocale } = browserLocaleInputs();
  return Boolean(
    resolveLocale({ cookieHeader, storageLocale, defaultLocale: "" }) !==
      DEFAULT_LOCALE ||
    normalizeLocale(storageLocale) ||
    String(cookieHeader ?? "").includes(`${LOCALE_COOKIE_NAME}=`),
  );
};

export const applyDocumentLocale = (locale: LocaleCode) => {
  if (typeof document !== "undefined") {
    document.documentElement.lang = locale;
  }
};

export const detectBrowserLocale = (
  defaultLocale: string | null | undefined = DEFAULT_LOCALE,
): LocaleCode => {
  const { cookieHeader, storageLocale } = browserLocaleInputs();
  return resolveLocale({ cookieHeader, storageLocale, defaultLocale });
};

export const persistBrowserLocale = (locale: LocaleCode) => {
  applyDocumentLocale(locale);
  if (typeof document === "undefined") return;
  document.cookie = `${LOCALE_COOKIE_NAME}=${encodeURIComponent(locale)}; Path=/; Max-Age=31536000; SameSite=Lax`;
  try {
    globalThis.localStorage?.setItem(LOCALE_STORAGE_KEY, locale);
  } catch {
    // Storage can be disabled in hardened or private browser contexts. The
    // first-party cookie remains the cross-load preference.
  }
};

export const createScopedFnKnockI18n = async (
  scope: BrowserI18nScope,
  { defaultLocale }: Pick<CreateFnKnockI18nOptions, "defaultLocale"> = {},
) => {
  const preferredLocale = detectBrowserLocale(defaultLocale);
  let locale = preferredLocale;
  try {
    await ensureScopedLocaleReady(scope, preferredLocale);
  } catch (error) {
    if (preferredLocale === DEFAULT_LOCALE) throw error;

    // A persisted locale points at its own fingerprinted chunk. After an
    // upgrade, an embedded browser can retain that preference while holding a
    // stale document or a broken representation of the chunk. Keep the app
    // mountable by falling back to the default locale. Persisting the fallback
    // also prevents repeat failures in clients where browser storage is the
    // active locale source.
    console.warn(
      `[i18n] failed to load ${scope} locale "${preferredLocale}"; falling back to "${DEFAULT_LOCALE}"`,
      error,
    );
    await ensureScopedLocaleReady(scope, DEFAULT_LOCALE);
    locale = DEFAULT_LOCALE;
    persistBrowserLocale(DEFAULT_LOCALE);
  }
  setActiveBrowserLocale(scope, locale);
  applyDocumentLocale(locale);
  return createI18n({
    legacy: false,
    locale,
    fallbackLocale: false,
    messages: toVueLocaleMessages(getScopedLocaleMessages(scope, locale)),
  });
};

export const setFnKnockLocale = async (
  i18n: unknown,
  value: string | null | undefined,
): Promise<LocaleCode> => {
  const locale = normalizeLocale(value) ?? DEFAULT_LOCALE;
  const scope = getActiveBrowserScope();
  await ensureScopedLocaleReady(scope, locale);

  const target = getLocaleMessageTarget(i18n);
  const messages = await loadScopedLocaleMessages(scope, locale);
  target.setLocaleMessage(locale, toVueLocaleMessage(messages));

  target.locale.value = locale;
  setActiveBrowserLocale(scope, locale);
  persistBrowserLocale(locale);
  return locale;
};

export const browserT = (
  key: string,
  params?: MessageParams,
  defaultLocale?: string | null,
): string => translateLoadedBrowserMessage(key, params, defaultLocale);
