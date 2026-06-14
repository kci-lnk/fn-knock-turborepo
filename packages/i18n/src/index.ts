import { messages, type I18nMessageSchema } from "./locales";
import {
  DEFAULT_LOCALE,
  interpolateMessage,
  readMessagePath,
  type LocaleCode,
  type MessageParams,
} from "./core";

export * from "./core";

export const translate = (
  locale: LocaleCode,
  key: string,
  params?: MessageParams,
): string => {
  const value =
    readMessagePath(messages[locale], key) ??
    readMessagePath(messages[DEFAULT_LOCALE], key);
  if (typeof value !== "string") return key;
  return interpolateMessage(value, params);
};

export const getLocaleMessages = (): Record<LocaleCode, I18nMessageSchema> =>
  messages;

export { messages };
