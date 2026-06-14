import { createI18n } from "vue-i18n";
import {
  DEFAULT_LOCALE,
  type LocaleCode,
  type MessageParams,
  normalizeLocale,
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

export const createScopedFnKnockI18n = async (
  scope: BrowserI18nScope,
  { defaultLocale }: Pick<CreateFnKnockI18nOptions, "defaultLocale"> = {},
) => {
  const locale = detectBrowserLocale(defaultLocale);
  await ensureScopedLocaleReady(scope, locale);
  setActiveBrowserLocale(scope, locale);
  applyDocumentLocale(locale);
  return createI18n({
    legacy: false,
    locale,
    fallbackLocale: DEFAULT_LOCALE,
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
  const defaultMessages = await loadScopedLocaleMessages(scope, DEFAULT_LOCALE);
  target.setLocaleMessage(DEFAULT_LOCALE, toVueLocaleMessage(defaultMessages));
  if (locale !== DEFAULT_LOCALE) {
    const messages = await loadScopedLocaleMessages(scope, locale);
    target.setLocaleMessage(locale, toVueLocaleMessage(messages));
  }

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
