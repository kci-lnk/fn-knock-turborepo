import type {
  NotificationMessage,
  NotificationMessageAction,
  NotificationMessageFact,
} from "./types";
import {
  DEFAULT_LOCALE,
  type LocaleCode,
  normalizeLocale,
  translate,
} from "../../../../../packages/i18n/src";

const brandT = (locale: LocaleCode, key: string) =>
  translate(locale, `server.notifications.brand.${key}`);

const trimValue = (value: unknown) => String(value ?? "").trim();

const normalizeFact = (
  fact: NotificationMessageFact,
): NotificationMessageFact | null => {
  const label = trimValue(fact.label);
  const value = trimValue(fact.value);
  if (!label && !value) return null;
  return {
    label,
    value,
  };
};

const normalizeAction = (
  action: NotificationMessageAction,
): NotificationMessageAction | null => {
  const label = trimValue(action.label);
  const url = trimValue(action.url);
  if (!label || !url) return null;
  return {
    label,
    url,
  };
};

export const brandNotificationTitle = (
  title?: string,
  locale?: string | null,
) => {
  const resolvedLocale = normalizeLocale(locale) ?? DEFAULT_LOCALE;
  const brandPrefix = brandT(resolvedLocale, "prefix");
  const normalized = trimValue(title);
  if (!normalized) return brandT(resolvedLocale, "defaultTitle");
  if (normalized.startsWith(brandPrefix)) {
    return normalized;
  }
  return `${brandPrefix}${normalized}`;
};

export const normalizeNotificationMessage = (
  message: NotificationMessage,
  locale?: string | null,
): NotificationMessage => ({
  ...message,
  title: brandNotificationTitle(message.title, locale),
  summary: trimValue(message.summary),
  body_text: trimValue(message.body_text),
  body_markdown: trimValue(message.body_markdown),
  facts: (message.facts || [])
    .map((fact) => normalizeFact(fact))
    .filter((fact): fact is NotificationMessageFact => Boolean(fact)),
  actions: (message.actions || [])
    .map((action) => normalizeAction(action))
    .filter((action): action is NotificationMessageAction => Boolean(action)),
  mentions: Array.from(
    new Set((message.mentions || []).map((mention) => trimValue(mention))),
  ).filter(Boolean),
});
