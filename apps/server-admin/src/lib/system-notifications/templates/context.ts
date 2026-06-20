import {
  DEFAULT_LOCALE,
  type LocaleCode,
  normalizeLocale,
  translate,
} from "../../../../../../packages/i18n/src";

let activeNotificationLocale: LocaleCode = DEFAULT_LOCALE;

export const getActiveNotificationLocale = (): LocaleCode =>
  activeNotificationLocale;

export const withNotificationLocale = <T>(
  locale: string | null | undefined,
  action: () => T,
): T => {
  const previous = activeNotificationLocale;
  activeNotificationLocale = normalizeLocale(locale) ?? DEFAULT_LOCALE;
  try {
    return action();
  } finally {
    activeNotificationLocale = previous;
  }
};

export const ntfT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
): string =>
  translate(
    activeNotificationLocale,
    `server.notifications.templates.${key}`,
    params,
  );

export const detailT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
): string => ntfT(`details.${key}`, params);

export const factLabel = (key: string): string => detailT(`facts.${key}`);
