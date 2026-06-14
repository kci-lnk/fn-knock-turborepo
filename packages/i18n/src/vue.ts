import { createI18n } from "vue-i18n";
import {
  DEFAULT_LOCALE,
  type LocaleCode,
  type MessageParams,
  getLocaleMessages,
  normalizeLocale,
  translate,
} from "./index";

export const hasBrowserLocalePreference = (): boolean => false;

export const applyDocumentLocale = (locale: LocaleCode) => {
  if (typeof document !== "undefined") {
    document.documentElement.lang = locale;
  }
};

export const detectBrowserLocale = (
  defaultLocale: string | null | undefined = DEFAULT_LOCALE,
): LocaleCode => normalizeLocale(defaultLocale) ?? DEFAULT_LOCALE;

export const persistBrowserLocale = (locale: LocaleCode) => {
  applyDocumentLocale(locale);
};

export const createFnKnockI18n = (defaultLocale?: string | null) => {
  const locale = detectBrowserLocale(defaultLocale);
  applyDocumentLocale(locale);
  return createI18n({
    legacy: false,
    locale,
    fallbackLocale: DEFAULT_LOCALE,
    messages: getLocaleMessages(),
  });
};

export const setFnKnockLocale = (
  i18n: ReturnType<typeof createFnKnockI18n>,
  value: string,
): LocaleCode => {
  const locale = normalizeLocale(value) ?? DEFAULT_LOCALE;
  i18n.global.locale.value = locale;
  persistBrowserLocale(locale);
  return locale;
};

export const browserT = (
  key: string,
  params?: MessageParams,
  defaultLocale?: string | null,
): string => translate(detectBrowserLocale(defaultLocale), key, params);
