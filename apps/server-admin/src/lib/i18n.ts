import {
  DEFAULT_LOCALE,
  type LocaleCode,
  type LocaleConfig,
  type MessageParams,
  normalizeLocale,
  translate,
} from "../../../../packages/i18n/src";

export const resolveRequestLocale = (
  request: Request,
  localeConfig?: Partial<LocaleConfig> | null,
): LocaleCode => normalizeLocale(localeConfig?.default_locale) ?? DEFAULT_LOCALE;

export const createRequestTranslator = (
  request: Request,
  localeConfig?: Partial<LocaleConfig> | null,
) => {
  const locale = resolveRequestLocale(request, localeConfig);
  return {
    locale,
    t: (
      key: string,
      params?: Record<string, string | number | boolean | null | undefined>,
    ) => translate(locale, key, params),
  };
};

export const tDefault = (key: string, params?: MessageParams): string =>
  translate(DEFAULT_LOCALE, key, params);

export const tWithFallback = (
  locale: LocaleCode,
  key: string,
  fallback: string,
  params?: MessageParams,
): string => {
  const translated = translate(locale, key, params);
  return translated === key ? fallback : translated;
};
