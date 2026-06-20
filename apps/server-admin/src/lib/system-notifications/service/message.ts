import {
  DEFAULT_LOCALE,
  normalizeLocale,
} from "../../../../../../packages/i18n/src";
import type { NotificationMessage } from "../types";
import { nowIso, serviceTForLocale } from "./common";

export const buildProviderTestMessage = (
  locale?: string | null,
): NotificationMessage => {
  const sentAt = nowIso();
  const t = (
    key: string,
    params?: Record<string, string | number | boolean | null | undefined>,
  ) => serviceTForLocale(locale, key, params);

  return {
    title: t("testMessage.title"),
    summary: t("testMessage.summary"),
    body_text: t("testMessage.bodyText"),
    body_markdown: t("testMessage.bodyMarkdown"),
    severity: "info",
    facts: [
      {
        label: t("testMessage.sendType"),
        value: t("testMessage.providerTest"),
      },
      {
        label: t("testMessage.sentAt"),
        value: sentAt,
      },
    ],
    actions: [],
    mentions: [],
    occurred_at: sentAt,
    metadata: {
      test: true,
      locale: normalizeLocale(locale) ?? DEFAULT_LOCALE,
    },
  };
};

export const resolveMessageLocale = (message: NotificationMessage) =>
  normalizeLocale(String(message.metadata?.locale ?? "")) ?? DEFAULT_LOCALE;
