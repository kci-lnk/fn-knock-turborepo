import type {
  NotificationDispatchContext,
  NotificationMessage,
  NotificationProvider,
  NotificationProviderDefinition,
  NotificationSchemaField,
  NotificationSendResult,
} from "../types";
import {
  escapeHtml,
  resolveOptionalStrictPositiveInteger,
  toPlainRecord,
  toTrimmedString,
  truncateText,
} from "./shared";
import { tDefault } from "../../i18n";

const telegramT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) =>
  tDefault(`server.notifications.providers.catalog.telegram.${key}`, params);

const TELEGRAM_CONNECTION_SCHEMA: NotificationSchemaField[] = [
  {
    key: "server_url",
    label: telegramT("fields.server_url.label"),
    description: telegramT("fields.server_url.description"),
    placeholder: "https://api.telegram.org",
    type: "string",
    required: true,
    default_value: "https://api.telegram.org",
  },
  {
    key: "bot_token",
    label: "Bot Token",
    description: telegramT("fields.bot_token.description"),
    placeholder: "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11",
    type: "string",
    required: true,
    sensitive: true,
  },
  {
    key: "chat_id",
    label: "Chat ID",
    description: telegramT("fields.chat_id.description"),
    placeholder: "-1001234567890",
    type: "string",
    required: true,
  },
  {
    key: "timeout_seconds",
    label: telegramT("fields.timeout_seconds.label"),
    type: "number",
    required: true,
    default_value: 5,
    min: 1,
    max: 30,
  },
];

const TELEGRAM_TARGET_SCHEMA: NotificationSchemaField[] = [
  {
    key: "message_thread_id",
    label: "Topic ID",
    description: telegramT("fields.message_thread_id.description"),
    type: "number",
    min: 1,
  },
  {
    key: "disable_notification",
    label: telegramT("fields.disable_notification.label"),
    description: telegramT("fields.disable_notification.description"),
    type: "boolean",
    default_value: false,
  },
];

export const telegramProviderDefinition: NotificationProviderDefinition = {
  type: "telegram",
  label: "Telegram",
  description: telegramT("description"),
  connection_schema: TELEGRAM_CONNECTION_SCHEMA,
  target_schema: TELEGRAM_TARGET_SCHEMA,
  sensitive_fields: ["bot_token"],
  capabilities: {
    supports_text: true,
    supports_markdown: false,
    supports_rich_blocks: false,
    supports_actions: true,
    supports_mentions: false,
    supports_attachments: false,
    supports_provider_dedupe_key: false,
    max_body_length: 4096,
  },
};

const resolveTelegramBaseUrl = (provider: NotificationProvider) => {
  const baseUrl = toTrimmedString(provider.connection_config.server_url);
  return (baseUrl || "https://api.telegram.org").replace(/\/+$/, "");
};

const buildTelegramText = (message: NotificationMessage) => {
  const plainSections: string[] = [];
  const richSections: string[] = [];
  const title = toTrimmedString(
    message.title || telegramT("message.fallbackTitle"),
  );
  const summary = toTrimmedString(message.summary);
  const bodyText = toTrimmedString(message.body_text);

  if (title) {
    plainSections.push(title);
    richSections.push(`<b>${escapeHtml(title)}</b>`);
  }
  if (summary) {
    plainSections.push(summary);
    richSections.push(escapeHtml(summary));
  }
  if (bodyText) {
    const normalizedBody = bodyText
      .split("\n")
      .map((line) => line.trim())
      .join("\n");
    plainSections.push(normalizedBody);
    richSections.push(
      normalizedBody
        .split("\n")
        .map((line) => escapeHtml(line))
        .join("\n"),
    );
  }
  if (message.facts.length > 0) {
    plainSections.push(
      message.facts.map((fact) => `${fact.label}: ${fact.value}`).join("\n"),
    );
    richSections.push(
      message.facts
        .map(
          (fact) =>
            `<b>${escapeHtml(fact.label)}:</b> ${escapeHtml(fact.value)}`,
        )
        .join("\n"),
    );
  }

  const richText = richSections.filter(Boolean).join("\n\n");
  if (richText.length <= 4096) {
    return richText;
  }

  return escapeHtml(
    truncateText(plainSections.filter(Boolean).join("\n\n"), 4096),
  );
};

const buildTelegramReplyMarkup = (message: NotificationMessage) => {
  const buttons = message.actions
    .filter(
      (action) => toTrimmedString(action.label) && toTrimmedString(action.url),
    )
    .map((action) => [
      {
        text: action.label.trim(),
        url: action.url.trim(),
      },
    ]);

  return buttons.length > 0
    ? {
        inline_keyboard: buttons,
      }
    : undefined;
};

export const sendTelegramMessage = async (args: {
  provider: NotificationProvider;
  message: NotificationMessage;
  context?: Partial<NotificationDispatchContext>;
  timeoutSeconds: number;
}): Promise<NotificationSendResult> => {
  const baseUrl = resolveTelegramBaseUrl(args.provider);
  const botToken = toTrimmedString(args.provider.connection_config.bot_token);
  const chatId = toTrimmedString(args.provider.connection_config.chat_id);
  if (!botToken) {
    return {
      success: false,
      retryable: false,
      reason: telegramT("errors.missingBotToken"),
    };
  }
  if (!chatId) {
    return {
      success: false,
      retryable: false,
      reason: telegramT("errors.missingChatId"),
    };
  }

  const targetConfig = toPlainRecord(args.context?.target?.target_config);
  const messageThreadId = resolveOptionalStrictPositiveInteger(
    targetConfig.message_thread_id,
  );
  const disableNotification = Boolean(targetConfig.disable_notification);
  const replyMarkup = buildTelegramReplyMarkup(args.message);
  const text = buildTelegramText(args.message);
  const url = `${baseUrl}/bot${botToken}/sendMessage`;
  const requestBody = {
    chat_id: chatId,
    text: text || telegramT("message.fallbackTitle"),
    parse_mode: "HTML",
    ...(messageThreadId ? { message_thread_id: messageThreadId } : {}),
    ...(disableNotification ? { disable_notification: true } : {}),
    ...(replyMarkup ? { reply_markup: replyMarkup } : {}),
  };

  const controller = new AbortController();
  const timeout = setTimeout(
    () => controller.abort(),
    Math.max(1, args.timeoutSeconds) * 1000,
  );

  try {
    const response = await fetch(url, {
      method: "POST",
      headers: {
        "content-type": "application/json; charset=utf-8",
      },
      body: JSON.stringify(requestBody),
      signal: controller.signal,
    });
    const responseText = await response.text().catch(() => "");
    let parsedResponse: {
      ok?: boolean;
      description?: string;
      error_code?: number;
      result?: {
        message_id?: number;
        chat?: {
          id?: number | string;
          title?: string;
          username?: string;
          type?: string;
        };
      };
    } | null = null;
    try {
      parsedResponse = responseText ? JSON.parse(responseText) : null;
    } catch {
      parsedResponse = null;
    }

    const apiOk = parsedResponse?.ok ?? response.ok;
    const apiErrorCode = parsedResponse?.error_code;
    const success = response.ok && apiOk;
    const retryable =
      !success &&
      (response.status >= 500 ||
        response.status === 429 ||
        apiErrorCode === 429);

    return {
      success,
      retryable,
      reason: success
        ? undefined
        : parsedResponse?.description ||
          telegramT("errors.requestReturned", { status: response.status }),
      request_summary: {
        method: "POST",
        url: `${baseUrl}/bot<redacted>/sendMessage`,
        chat_id: chatId,
        message_thread_id: messageThreadId,
        disable_notification: disableNotification,
        has_inline_keyboard: Boolean(replyMarkup),
        text_preview: truncateText(toTrimmedString(args.message.title), 120),
      },
      response_summary: {
        status: response.status,
        ok: response.ok,
        api_ok: parsedResponse?.ok,
        error_code: apiErrorCode,
        description: parsedResponse?.description,
        message_id: parsedResponse?.result?.message_id,
        chat: parsedResponse?.result?.chat,
        body_preview: truncateText(responseText),
      },
    };
  } catch (error) {
    return {
      success: false,
      retryable: true,
      reason:
        error instanceof Error
          ? error.message
          : telegramT("errors.requestFailed"),
      request_summary: {
        method: "POST",
        url: `${baseUrl}/bot<redacted>/sendMessage`,
        chat_id: chatId,
        message_thread_id: messageThreadId,
        disable_notification: disableNotification,
        has_inline_keyboard: Boolean(replyMarkup),
      },
    };
  } finally {
    clearTimeout(timeout);
  }
};
