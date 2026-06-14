import type {
  NotificationDispatchContext,
  NotificationMessage,
  NotificationProvider,
  NotificationProviderDefinition,
  NotificationSchemaField,
  NotificationSendResult,
} from "../types";
import {
  getUtf8ByteLength,
  splitCommaSeparatedValues,
  toPlainRecord,
  toTrimmedString,
  truncateText,
  truncateUtf8ByBytes,
} from "./shared";
import { tDefault } from "../../i18n";

const wecomT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.notifications.providers.catalog.wecom.${key}`, params);

const WECOM_CONNECTION_SCHEMA: NotificationSchemaField[] = [
  {
    key: "webhook_url",
    label: "Webhook URL",
    description: wecomT("fields.webhook_url.description"),
    placeholder:
      "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
    type: "string",
    required: true,
    sensitive: true,
  },
  {
    key: "timeout_seconds",
    label: wecomT("fields.timeout_seconds.label"),
    type: "number",
    required: true,
    default_value: 5,
    min: 1,
    max: 30,
  },
];

const WECOM_TARGET_SCHEMA: NotificationSchemaField[] = [
  {
    key: "mentioned_list",
    label: wecomT("fields.mentioned_list.label"),
    description: wecomT("fields.mentioned_list.description"),
    placeholder: "zhangsan,@all",
    type: "string",
  },
  {
    key: "mentioned_mobile_list",
    label: wecomT("fields.mentioned_mobile_list.label"),
    description: wecomT("fields.mentioned_mobile_list.description"),
    placeholder: "13800001111,@all",
    type: "string",
  },
];

export const wecomProviderDefinition: NotificationProviderDefinition = {
  type: "wecom",
  label: wecomT("label"),
  description: wecomT("description"),
  connection_schema: WECOM_CONNECTION_SCHEMA,
  target_schema: WECOM_TARGET_SCHEMA,
  sensitive_fields: ["webhook_url"],
  capabilities: {
    supports_text: true,
    supports_markdown: true,
    supports_rich_blocks: false,
    supports_actions: true,
    supports_mentions: true,
    supports_attachments: false,
    supports_provider_dedupe_key: false,
    max_body_length: 4096,
  },
};

const sanitizeWecomText = (value: string) =>
  value.replaceAll("<", "＜").replaceAll(">", "＞");

const resolveWecomWebhookUrl = (provider: NotificationProvider) =>
  toTrimmedString(provider.connection_config.webhook_url);

const redactWecomWebhookUrl = (value: string) => {
  try {
    const url = new URL(value);
    if (url.searchParams.has("key")) {
      url.searchParams.set("key", "<redacted>");
    }
    return url.toString();
  } catch {
    return value.replace(/key=[^&]+/i, "key=<redacted>");
  }
};

const toWecomInlineMention = (value: string) =>
  value.startsWith("@") ? `<${value}>` : `<@${value}>`;

const buildWecomMarkdownContent = (
  message: NotificationMessage,
  mentionedList: string[],
) => {
  const sections: string[] = [];
  const title = toTrimmedString(
    message.title || wecomT("message.fallbackTitle"),
  );
  const summary = toTrimmedString(message.summary);
  const bodyText = toTrimmedString(message.body_text);

  if (title) {
    sections.push(`# ${sanitizeWecomText(title)}`);
  }
  if (summary) {
    sections.push(sanitizeWecomText(summary));
  }
  if (bodyText) {
    sections.push(
      bodyText
        .split("\n")
        .map((line) => sanitizeWecomText(line.trim()))
        .filter(Boolean)
        .join("\n"),
    );
  }
  if (message.facts.length > 0) {
    sections.push(
      message.facts
        .map(
          (fact) =>
            `> ${sanitizeWecomText(fact.label)}：${sanitizeWecomText(fact.value)}`,
        )
        .join("\n"),
    );
  }
  if (message.actions.length > 0) {
    sections.push(
      message.actions
        .filter(
          (action) =>
            toTrimmedString(action.label) && toTrimmedString(action.url),
        )
        .map(
          (action) =>
            `> ${sanitizeWecomText(action.label.trim())}：${action.url.trim()}`,
        )
        .join("\n"),
    );
  }
  if (mentionedList.length > 0) {
    sections.push(
      mentionedList.map((item) => toWecomInlineMention(item)).join(" "),
    );
  }

  return sections.filter(Boolean).join("\n\n");
};

const buildWecomTextContent = (message: NotificationMessage) => {
  const sections: string[] = [];
  const title = toTrimmedString(
    message.title || wecomT("message.fallbackTitle"),
  );
  const summary = toTrimmedString(message.summary);
  const bodyText = toTrimmedString(message.body_text);

  if (title) {
    sections.push(sanitizeWecomText(title));
  }
  if (summary) {
    sections.push(sanitizeWecomText(summary));
  }
  if (bodyText) {
    sections.push(
      bodyText
        .split("\n")
        .map((line) => sanitizeWecomText(line.trim()))
        .filter(Boolean)
        .join("\n"),
    );
  }
  if (message.facts.length > 0) {
    sections.push(
      message.facts
        .map(
          (fact) =>
            `${sanitizeWecomText(fact.label)}：${sanitizeWecomText(fact.value)}`,
        )
        .join("\n"),
    );
  }
  if (message.actions.length > 0) {
    sections.push(
      message.actions
        .filter(
          (action) =>
            toTrimmedString(action.label) && toTrimmedString(action.url),
        )
        .map(
          (action) =>
            `${sanitizeWecomText(action.label.trim())}：${action.url.trim()}`,
        )
        .join("\n"),
    );
  }

  return sections.filter(Boolean).join("\n\n");
};

export const sendWecomMessage = async (args: {
  provider: NotificationProvider;
  message: NotificationMessage;
  context?: Partial<NotificationDispatchContext>;
  timeoutSeconds: number;
}): Promise<NotificationSendResult> => {
  const webhookUrl = resolveWecomWebhookUrl(args.provider);
  if (!webhookUrl) {
    return {
      success: false,
      retryable: false,
      reason: wecomT("errors.missingWebhookUrl"),
    };
  }

  const targetConfig = toPlainRecord(args.context?.target?.target_config);
  const mentionedList = splitCommaSeparatedValues(targetConfig.mentioned_list);
  const mentionedMobileList = splitCommaSeparatedValues(
    targetConfig.mentioned_mobile_list,
  );
  const markdownContent = buildWecomMarkdownContent(
    args.message,
    mentionedList,
  );
  const useTextPayload =
    mentionedMobileList.length > 0 || getUtf8ByteLength(markdownContent) > 4096;
  const requestBody = useTextPayload
    ? {
        msgtype: "text",
        text: {
          content:
            truncateUtf8ByBytes(buildWecomTextContent(args.message), 2048) ||
            wecomT("message.fallbackTitle"),
          ...(mentionedList.length > 0
            ? { mentioned_list: mentionedList }
            : {}),
          ...(mentionedMobileList.length > 0
            ? { mentioned_mobile_list: mentionedMobileList }
            : {}),
        },
      }
    : {
        msgtype: "markdown",
        markdown: {
          content:
            truncateUtf8ByBytes(markdownContent, 4096) ||
            wecomT("message.fallbackTitle"),
        },
      };

  const controller = new AbortController();
  const timeout = setTimeout(
    () => controller.abort(),
    Math.max(1, args.timeoutSeconds) * 1000,
  );

  try {
    const response = await fetch(webhookUrl, {
      method: "POST",
      headers: {
        "content-type": "application/json; charset=utf-8",
      },
      body: JSON.stringify(requestBody),
      signal: controller.signal,
    });
    const responseText = await response.text().catch(() => "");
    let parsedResponse: {
      errcode?: number;
      errmsg?: string;
    } | null = null;
    try {
      parsedResponse = responseText ? JSON.parse(responseText) : null;
    } catch {
      parsedResponse = null;
    }

    const apiSucceeded = response.ok && (parsedResponse?.errcode ?? 0) === 0;

    return {
      success: apiSucceeded,
      retryable:
        !apiSucceeded && (response.status >= 500 || response.status === 429),
      reason: apiSucceeded
        ? undefined
        : parsedResponse?.errmsg ||
          wecomT("errors.requestReturned", { status: response.status }),
      request_summary: {
        method: "POST",
        url: redactWecomWebhookUrl(webhookUrl),
        msgtype: requestBody.msgtype,
        mentioned_count: mentionedList.length,
        mentioned_mobile_count: mentionedMobileList.length,
      },
      response_summary: {
        status: response.status,
        ok: response.ok,
        errcode: parsedResponse?.errcode,
        errmsg: parsedResponse?.errmsg,
        body_preview: truncateText(responseText),
      },
    };
  } catch (error) {
    return {
      success: false,
      retryable: true,
      reason: error instanceof Error ? error.message : wecomT("errors.requestFailed"),
      request_summary: {
        method: "POST",
        url: redactWecomWebhookUrl(webhookUrl),
        msgtype: requestBody.msgtype,
      },
    };
  } finally {
    clearTimeout(timeout);
  }
};
