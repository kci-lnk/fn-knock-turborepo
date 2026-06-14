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
  toPlainRecord,
  toTrimmedString,
  truncateText,
} from "./shared";
import { tDefault } from "../../i18n";

const pushplusT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) =>
  tDefault(`server.notifications.providers.catalog.pushplus.${key}`, params);

const PUSHPLUS_CONNECTION_SCHEMA: NotificationSchemaField[] = [
  {
    key: "server_url",
    label: pushplusT("fields.server_url.label"),
    description: pushplusT("fields.server_url.description"),
    placeholder: "https://www.pushplus.plus",
    type: "string",
    required: true,
    default_value: "https://www.pushplus.plus",
  },
  {
    key: "token",
    label: "Token",
    description: pushplusT("fields.token.description"),
    placeholder: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
    type: "string",
    required: true,
    sensitive: true,
  },
  {
    key: "timeout_seconds",
    label: pushplusT("fields.timeout_seconds.label"),
    type: "number",
    required: true,
    default_value: 5,
    min: 1,
    max: 30,
  },
];

const PUSHPLUS_TARGET_SCHEMA: NotificationSchemaField[] = [
  {
    key: "topic",
    label: pushplusT("fields.topic.label"),
    description: pushplusT("fields.topic.description"),
    placeholder: "alarm-topic",
    type: "string",
  },
  {
    key: "template",
    label: pushplusT("fields.template.label"),
    description: pushplusT("fields.template.description"),
    type: "select",
    default_value: "markdown",
    options: [
      { label: "Markdown", value: "markdown" },
      { label: "HTML", value: "html" },
      { label: pushplusT("fields.template.options.txt"), value: "txt" },
      { label: "JSON", value: "json" },
    ],
  },
  {
    key: "channel",
    label: pushplusT("fields.channel.label"),
    description: pushplusT("fields.channel.description"),
    type: "select",
    default_value: "wechat",
    options: [
      { label: pushplusT("fields.channel.options.wechat"), value: "wechat" },
      { label: pushplusT("fields.channel.options.webhook"), value: "webhook" },
      { label: pushplusT("fields.channel.options.cp"), value: "cp" },
      { label: pushplusT("fields.channel.options.mail"), value: "mail" },
      { label: pushplusT("fields.channel.options.sms"), value: "sms" },
      { label: pushplusT("fields.channel.options.voice"), value: "voice" },
      {
        label: pushplusT("fields.channel.options.extension"),
        value: "extension",
      },
      { label: "App", value: "app" },
      { label: pushplusT("fields.channel.options.clawbot"), value: "clawbot" },
    ],
  },
  {
    key: "option",
    label: pushplusT("fields.option.label"),
    description: pushplusT("fields.option.description"),
    placeholder: "my-channel-code",
    type: "string",
  },
  {
    key: "to",
    label: pushplusT("fields.to.label"),
    description: pushplusT("fields.to.description"),
    placeholder: pushplusT("fields.to.placeholder"),
    type: "string",
  },
  {
    key: "callback_url",
    label: pushplusT("fields.callback_url.label"),
    description: pushplusT("fields.callback_url.description"),
    placeholder: "https://example.com/hooks/pushplus",
    type: "string",
  },
  {
    key: "pre",
    label: pushplusT("fields.pre.label"),
    description: pushplusT("fields.pre.description"),
    placeholder: "appendMsg",
    type: "string",
  },
];

export const pushplusProviderDefinition: NotificationProviderDefinition = {
  type: "pushplus",
  label: "PushPlus",
  description: pushplusT("description"),
  connection_schema: PUSHPLUS_CONNECTION_SCHEMA,
  target_schema: PUSHPLUS_TARGET_SCHEMA,
  sensitive_fields: ["token"],
  capabilities: {
    supports_text: true,
    supports_markdown: true,
    supports_rich_blocks: false,
    supports_actions: true,
    supports_mentions: false,
    supports_attachments: false,
    supports_provider_dedupe_key: false,
    max_body_length: null,
  },
};

const PUSHPLUS_TEMPLATE_VALUES = ["html", "txt", "json", "markdown"] as const;
type PushPlusTemplate = (typeof PUSHPLUS_TEMPLATE_VALUES)[number];

const resolvePushPlusUrl = (provider: NotificationProvider) => {
  const baseUrl = toTrimmedString(provider.connection_config.server_url);
  const normalizedBaseUrl = baseUrl || "https://www.pushplus.plus";
  if (/\/(?:send|batchSend)\/?$/i.test(normalizedBaseUrl)) {
    return normalizedBaseUrl;
  }
  return `${normalizedBaseUrl.replace(/\/+$/, "")}/send`;
};

const buildPushPlusTextContent = (message: NotificationMessage) => {
  const sections: string[] = [];

  if (message.summary?.trim()) {
    sections.push(message.summary.trim());
  }

  if (message.body_text?.trim()) {
    sections.push(
      message.body_text
        .trim()
        .split("\n")
        .map((line) => line.trim())
        .filter(Boolean)
        .join("\n"),
    );
  } else if (message.body_markdown?.trim()) {
    sections.push(message.body_markdown.trim());
  }

  if (message.facts.length > 0) {
    sections.push(
      message.facts.map((fact) => `${fact.label}：${fact.value}`).join("\n"),
    );
  }

  if (message.actions.length > 0) {
    sections.push(
      message.actions
        .filter(
          (action) =>
            toTrimmedString(action.label) && toTrimmedString(action.url),
        )
        .map((action) => `${action.label.trim()}：${action.url.trim()}`)
        .join("\n"),
    );
  }

  return (
    sections.filter(Boolean).join("\n\n") ||
    toTrimmedString(
      message.title || message.summary || pushplusT("message.fallbackTitle"),
    )
  );
};

const buildPushPlusMarkdownContent = (message: NotificationMessage) => {
  const sections: string[] = [];

  if (message.summary?.trim()) {
    sections.push(message.summary.trim());
  }

  if (message.body_markdown?.trim()) {
    sections.push(message.body_markdown.trim());
  } else if (message.body_text?.trim()) {
    sections.push(
      message.body_text
        .trim()
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

  return (
    sections.filter(Boolean).join("\n\n") || buildPushPlusTextContent(message)
  );
};

const buildPushPlusHtmlContent = (message: NotificationMessage) => {
  const sections: string[] = [];

  if (message.summary?.trim()) {
    sections.push(`<p>${escapeHtml(message.summary.trim())}</p>`);
  }

  if (message.body_text?.trim()) {
    sections.push(
      `<p>${message.body_text
        .trim()
        .split("\n")
        .map((line) => escapeHtml(line.trim()))
        .filter(Boolean)
        .join("<br />")}</p>`,
    );
  } else if (message.body_markdown?.trim()) {
    sections.push(`<pre>${escapeHtml(message.body_markdown.trim())}</pre>`);
  }

  if (message.facts.length > 0) {
    sections.push(
      `<ul>${message.facts
        .map(
          (fact) =>
            `<li><strong>${escapeHtml(fact.label)}</strong>：${escapeHtml(fact.value)}</li>`,
        )
        .join("")}</ul>`,
    );
  }

  if (message.actions.length > 0) {
    sections.push(
      `<ul>${message.actions
        .filter(
          (action) =>
            toTrimmedString(action.label) && toTrimmedString(action.url),
        )
        .map(
          (action) =>
            `<li><a href="${escapeHtml(action.url.trim())}">${escapeHtml(action.label.trim())}</a></li>`,
        )
        .join("")}</ul>`,
    );
  }

  return (
    sections.filter(Boolean).join("") ||
    `<p>${escapeHtml(
      toTrimmedString(
        message.title || message.summary || pushplusT("message.fallbackTitle"),
      ),
    )}</p>`
  );
};

const buildPushPlusJsonContent = (message: NotificationMessage) =>
  JSON.stringify(
    {
      summary: message.summary,
      body_text: message.body_text,
      body_markdown: message.body_markdown,
      severity: message.severity,
      facts: message.facts,
      actions: message.actions,
      occurred_at: message.occurred_at,
      event_id: message.event_id,
      metadata: message.metadata,
    },
    null,
    2,
  );

const parsePushPlusApiCode = (response: Record<string, unknown> | null) => {
  const rawCode = response?.code;
  if (typeof rawCode === "number") {
    return rawCode;
  }

  const parsed = Number.parseInt(String(rawCode ?? ""), 10);
  return Number.isFinite(parsed) ? parsed : undefined;
};

const resolvePushPlusTemplate = (value: unknown): PushPlusTemplate => {
  const template = toTrimmedString(value).toLowerCase();
  return PUSHPLUS_TEMPLATE_VALUES.includes(template as PushPlusTemplate)
    ? (template as PushPlusTemplate)
    : "markdown";
};

const isPushPlusRetryable = (status: number, apiCode?: number) =>
  status === 429 || status >= 500 || apiCode === 500 || apiCode === 999;

export const sendPushPlusMessage = async (args: {
  provider: NotificationProvider;
  message: NotificationMessage;
  context?: Partial<NotificationDispatchContext>;
  timeoutSeconds: number;
}): Promise<NotificationSendResult> => {
  const providerConfig = toPlainRecord(args.provider.connection_config);
  const targetConfig = toPlainRecord(args.context?.target?.target_config);
  const token = toTrimmedString(providerConfig.token);

  if (!token) {
    return {
      success: false,
      retryable: false,
      reason: pushplusT("errors.missingToken"),
    };
  }

  const url = resolvePushPlusUrl(args.provider);
  const template = resolvePushPlusTemplate(targetConfig.template);
  const channel = toTrimmedString(targetConfig.channel) || "wechat";
  const topic = toTrimmedString(targetConfig.topic);
  const option = toTrimmedString(targetConfig.option);
  const to = toTrimmedString(targetConfig.to);
  const callbackUrl = toTrimmedString(targetConfig.callback_url);
  const pre = toTrimmedString(targetConfig.pre);
  const title = truncateText(
    toTrimmedString(
      args.message.title ||
        args.message.summary ||
        pushplusT("message.fallbackTitle"),
    ),
    128,
  );

  const content =
    template === "html"
      ? buildPushPlusHtmlContent(args.message)
      : template === "txt"
        ? buildPushPlusTextContent(args.message)
        : template === "json"
          ? buildPushPlusJsonContent(args.message)
          : buildPushPlusMarkdownContent(args.message);

  const requestBody = {
    token,
    ...(title ? { title } : {}),
    content: content || pushplusT("message.fallbackTitle"),
    template,
    channel,
    ...(topic ? { topic } : {}),
    ...(option ? { option } : {}),
    ...(callbackUrl ? { callbackUrl } : {}),
    ...(to ? { to } : {}),
    ...(pre ? { pre } : {}),
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
    let parsedResponse: Record<string, unknown> | null = null;
    try {
      parsedResponse = responseText
        ? (JSON.parse(responseText) as Record<string, unknown>)
        : null;
    } catch {
      parsedResponse = null;
    }

    const apiCode = parsePushPlusApiCode(parsedResponse);
    const apiMessage = toTrimmedString(
      parsedResponse?.msg ?? parsedResponse?.message ?? parsedResponse?.error,
    );
    const succeeded = response.ok && apiCode === 200;

    return {
      success: succeeded,
      retryable: !succeeded && isPushPlusRetryable(response.status, apiCode),
      reason: succeeded
        ? undefined
        : apiMessage || `PushPlus returned ${response.status}`,
      request_summary: {
        method: "POST",
        endpoint: url,
        channel,
        template,
        has_topic: Boolean(topic),
        has_option: Boolean(option),
        has_to: Boolean(to),
        has_callback_url: Boolean(callbackUrl),
        has_pre: Boolean(pre),
        title_preview: title,
      },
      response_summary: {
        status: response.status,
        ok: response.ok,
        code: apiCode,
        message: apiMessage || undefined,
        short_code:
          typeof parsedResponse?.data === "string"
            ? parsedResponse.data
            : undefined,
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
          : pushplusT("errors.requestFailed"),
      request_summary: {
        method: "POST",
        endpoint: url,
        channel,
        template,
        has_topic: Boolean(topic),
        has_option: Boolean(option),
        has_to: Boolean(to),
        has_callback_url: Boolean(callbackUrl),
        has_pre: Boolean(pre),
        title_preview: title,
      },
    };
  } finally {
    clearTimeout(timeout);
  }
};
