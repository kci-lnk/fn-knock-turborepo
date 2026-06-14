import { createHmac } from "node:crypto";
import type {
  NotificationDispatchContext,
  NotificationMessage,
  NotificationProvider,
  NotificationProviderDefinition,
  NotificationSchemaField,
  NotificationSendResult,
} from "../types";
import {
  splitCommaSeparatedValues,
  toPlainRecord,
  toTrimmedString,
  truncateText,
} from "./shared";
import { tDefault } from "../../i18n";

const dingtalkT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) =>
  tDefault(`server.notifications.providers.catalog.dingtalk.${key}`, params);

const DINGTALK_CONNECTION_SCHEMA: NotificationSchemaField[] = [
  {
    key: "webhook_url",
    label: "Webhook URL",
    description: dingtalkT("fields.webhook_url.description"),
    placeholder: "https://oapi.dingtalk.com/robot/send?access_token=xxxxxx",
    type: "string",
    required: true,
    sensitive: true,
  },
  {
    key: "secret",
    label: dingtalkT("fields.secret.label"),
    description: dingtalkT("fields.secret.description"),
    placeholder: "SECxxxxxxxx",
    type: "string",
    sensitive: true,
  },
  {
    key: "keyword_prefix",
    label: dingtalkT("fields.keyword_prefix.label"),
    description: dingtalkT("fields.keyword_prefix.description"),
    placeholder: dingtalkT("fields.keyword_prefix.placeholder"),
    type: "string",
  },
  {
    key: "timeout_seconds",
    label: dingtalkT("fields.timeout_seconds.label"),
    type: "number",
    required: true,
    default_value: 5,
    min: 1,
    max: 30,
  },
];

const DINGTALK_TARGET_SCHEMA: NotificationSchemaField[] = [
  {
    key: "at_mobiles",
    label: dingtalkT("fields.at_mobiles.label"),
    description: dingtalkT("fields.at_mobiles.description"),
    placeholder: "13800001111,13900002222",
    type: "string",
  },
  {
    key: "at_user_ids",
    label: dingtalkT("fields.at_user_ids.label"),
    description: dingtalkT("fields.at_user_ids.description"),
    placeholder: "manager7675,user123",
    type: "string",
  },
  {
    key: "is_at_all",
    label: dingtalkT("fields.is_at_all.label"),
    description: dingtalkT("fields.is_at_all.description"),
    type: "boolean",
    default_value: false,
  },
];

export const dingtalkProviderDefinition: NotificationProviderDefinition = {
  type: "dingtalk",
  label: dingtalkT("label"),
  description: dingtalkT("description"),
  connection_schema: DINGTALK_CONNECTION_SCHEMA,
  target_schema: DINGTALK_TARGET_SCHEMA,
  sensitive_fields: ["webhook_url", "secret"],
  capabilities: {
    supports_text: true,
    supports_markdown: true,
    supports_rich_blocks: false,
    supports_actions: true,
    supports_mentions: true,
    supports_attachments: false,
    supports_provider_dedupe_key: false,
    max_body_length: null,
  },
};

const applyKeywordPrefix = (value: string, keyword: string) => {
  const trimmedKeyword = keyword.trim();
  const trimmedValue = value.trim();
  if (!trimmedKeyword) return trimmedValue;
  if (trimmedValue.includes(trimmedKeyword)) return trimmedValue;
  return trimmedValue
    ? `【${trimmedKeyword}】 ${trimmedValue}`
    : trimmedKeyword;
};

const buildQueryUrl = (url: string, params: Record<string, string>) => {
  try {
    const parsed = new URL(url);
    for (const [key, value] of Object.entries(params)) {
      parsed.searchParams.set(key, value);
    }
    return parsed.toString();
  } catch {
    const query = Object.entries(params)
      .map(
        ([key, value]) =>
          `${encodeURIComponent(key)}=${encodeURIComponent(value)}`,
      )
      .join("&");
    return `${url}${url.includes("?") ? "&" : "?"}${query}`;
  }
};

const redactDingTalkWebhookUrl = (value: string) => {
  try {
    const url = new URL(value);
    if (url.searchParams.has("access_token")) {
      url.searchParams.set("access_token", "<redacted>");
    }
    if (url.searchParams.has("sign")) {
      url.searchParams.set("sign", "<redacted>");
    }
    return url.toString();
  } catch {
    return value
      .replace(/access_token=[^&]+/gi, "access_token=<redacted>")
      .replace(/sign=[^&]+/gi, "sign=<redacted>");
  }
};

const buildDingTalkMentionText = (
  atMobiles: string[],
  atUserIds: string[],
  isAtAll: boolean,
) => {
  const tokens = [
    ...atMobiles.map((mobile) => `@${mobile}`),
    ...atUserIds.map((userId) => `@${userId}`),
  ];

  if (isAtAll) {
    tokens.unshift(dingtalkT("mentionAll"));
  }

  return tokens.join(" ").trim();
};

const buildDingTalkMarkdownText = (
  message: NotificationMessage,
  mentionText: string,
) => {
  const sections: string[] = [];
  const summary = toTrimmedString(message.summary);
  const bodySource =
    toTrimmedString(message.body_markdown) ||
    toTrimmedString(message.body_text);

  if (summary) {
    sections.push(summary);
  }
  if (bodySource) {
    sections.push(
      bodySource
        .split("\n")
        .map((line) => line.trim())
        .filter(Boolean)
        .join("\n"),
    );
  }
  if (message.facts.length > 0) {
    sections.push(
      message.facts
        .map((fact) => `- **${fact.label}**：${fact.value}`)
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
        .map((action) => `- [${action.label.trim()}](${action.url.trim()})`)
        .join("\n"),
    );
  }
  if (mentionText) {
    sections.push(mentionText);
  }

  return sections.filter(Boolean).join("\n\n");
};

export const sendDingTalkMessage = async (args: {
  provider: NotificationProvider;
  message: NotificationMessage;
  context?: Partial<NotificationDispatchContext>;
  timeoutSeconds: number;
}): Promise<NotificationSendResult> => {
  const webhookUrl = toTrimmedString(
    args.provider.connection_config.webhook_url,
  );
  if (!webhookUrl) {
    return {
      success: false,
      retryable: false,
      reason: dingtalkT("errors.missingWebhookUrl"),
    };
  }

  const secret = toTrimmedString(args.provider.connection_config.secret);
  const keywordPrefix = toTrimmedString(
    args.provider.connection_config.keyword_prefix,
  );
  const targetConfig = toPlainRecord(args.context?.target?.target_config);
  const atMobiles = splitCommaSeparatedValues(targetConfig.at_mobiles);
  const atUserIds = splitCommaSeparatedValues(targetConfig.at_user_ids);
  const isAtAll = Boolean(targetConfig.is_at_all);
  const mentionText = buildDingTalkMentionText(atMobiles, atUserIds, isAtAll);
  const title = applyKeywordPrefix(
    toTrimmedString(args.message.title || dingtalkT("message.fallbackTitle")),
    keywordPrefix,
  );
  const markdownText =
    buildDingTalkMarkdownText(args.message, mentionText) ||
    toTrimmedString(args.message.summary) ||
    title;

  const timestamp = secret ? String(Date.now()) : "";
  const sign = secret
    ? createHmac("sha256", secret)
        .update(`${timestamp}\n${secret}`, "utf8")
        .digest("base64")
    : "";
  const requestUrl =
    secret && timestamp && sign
      ? buildQueryUrl(webhookUrl, { timestamp, sign })
      : webhookUrl;
  const requestBody = {
    msgtype: "markdown",
    markdown: {
      title,
      text: markdownText,
    },
    ...(atMobiles.length > 0 || atUserIds.length > 0 || isAtAll
      ? {
          at: {
            ...(atMobiles.length > 0 ? { atMobiles } : {}),
            ...(atUserIds.length > 0 ? { atUserIds } : {}),
            isAtAll,
          },
        }
      : {}),
  };

  const controller = new AbortController();
  const timeout = setTimeout(
    () => controller.abort(),
    Math.max(1, args.timeoutSeconds) * 1000,
  );

  try {
    const response = await fetch(requestUrl, {
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
          dingtalkT("errors.requestReturned", { status: response.status }),
      request_summary: {
        method: "POST",
        url: redactDingTalkWebhookUrl(requestUrl),
        msgtype: requestBody.msgtype,
        signed: Boolean(secret),
        mentioned_mobile_count: atMobiles.length,
        mentioned_user_count: atUserIds.length,
        is_at_all: isAtAll,
        title_preview: truncateText(title, 120),
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
      reason:
        error instanceof Error
          ? error.message
          : dingtalkT("errors.requestFailed"),
      request_summary: {
        method: "POST",
        url: redactDingTalkWebhookUrl(requestUrl),
        msgtype: requestBody.msgtype,
        signed: Boolean(secret),
      },
    };
  } finally {
    clearTimeout(timeout);
  }
};
