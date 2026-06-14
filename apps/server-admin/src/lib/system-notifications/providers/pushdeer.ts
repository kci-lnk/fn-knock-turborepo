import type {
  NotificationMessage,
  NotificationProvider,
  NotificationProviderDefinition,
  NotificationSchemaField,
  NotificationSendResult,
} from "../types";
import { truncateText } from "./shared";
import { tDefault } from "../../i18n";

const pushdeerT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) =>
  tDefault(`server.notifications.providers.catalog.pushdeer.${key}`, params);

const PUSHDEER_CONNECTION_SCHEMA: NotificationSchemaField[] = [
  {
    key: "server_url",
    label: pushdeerT("fields.server_url.label"),
    description: pushdeerT("fields.server_url.description"),
    placeholder: "https://api2.pushdeer.com",
    type: "string",
    required: true,
    default_value: "https://api2.pushdeer.com",
  },
  {
    key: "pushkey",
    label: "PushKey",
    description: pushdeerT("fields.pushkey.description"),
    placeholder: "PDUxxxx,PDUyyyy",
    type: "string",
    required: true,
    sensitive: true,
  },
  {
    key: "timeout_seconds",
    label: pushdeerT("fields.timeout_seconds.label"),
    type: "number",
    required: true,
    default_value: 5,
    min: 1,
    max: 30,
  },
];

const PUSHDEER_TARGET_SCHEMA: NotificationSchemaField[] = [];

export const pushdeerProviderDefinition: NotificationProviderDefinition = {
  type: "pushdeer",
  label: "PushDeer",
  description: pushdeerT("description"),
  connection_schema: PUSHDEER_CONNECTION_SCHEMA,
  target_schema: PUSHDEER_TARGET_SCHEMA,
  sensitive_fields: ["pushkey"],
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

const buildPushDeerMarkdownBody = (message: NotificationMessage) => {
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
        .map((action) => `- [${action.label}](${action.url})`)
        .join("\n"),
    );
  }

  return sections.filter(Boolean).join("\n\n");
};

const resolvePushDeerUrl = (provider: NotificationProvider) => {
  const baseUrl = String(
    provider.connection_config.server_url || "https://api2.pushdeer.com",
  ).trim();
  const normalizedBaseUrl = baseUrl || "https://api2.pushdeer.com";
  return `${normalizedBaseUrl.replace(/\/+$/, "")}/message/push`;
};

export const sendPushDeerMessage = async (args: {
  provider: NotificationProvider;
  message: NotificationMessage;
  timeoutSeconds: number;
}): Promise<NotificationSendResult> => {
  const url = resolvePushDeerUrl(args.provider);
  const pushkey = String(args.provider.connection_config.pushkey || "").trim();
  if (!pushkey) {
    return {
      success: false,
      retryable: false,
      reason: pushdeerT("errors.missingPushKey"),
    };
  }

  const body = new URLSearchParams({
    pushkey,
    text:
      args.message.title ||
      args.message.summary ||
      pushdeerT("message.fallbackTitle"),
    desp: buildPushDeerMarkdownBody(args.message),
    type: "markdown",
  });

  const controller = new AbortController();
  const timeout = setTimeout(
    () => controller.abort(),
    Math.max(1, args.timeoutSeconds) * 1000,
  );

  try {
    const response = await fetch(url, {
      method: "POST",
      headers: {
        "content-type": "application/x-www-form-urlencoded",
      },
      body,
      signal: controller.signal,
    });
    const responseText = await response.text().catch(() => "");
    let parsedResponse: {
      code?: number;
      content?: unknown;
      error?: string;
    } | null = null;
    try {
      parsedResponse = responseText ? JSON.parse(responseText) : null;
    } catch {
      parsedResponse = null;
    }

    const requestSummary = {
      method: "POST",
      url,
      pushkey_count: pushkey
        .split(",")
        .map((item) => item.trim())
        .filter(Boolean).length,
      type: "markdown",
      title_preview: args.message.title,
    };
    const responseSummary = {
      status: response.status,
      ok: response.ok,
      code: parsedResponse?.code,
      error: parsedResponse?.error,
      body_preview: truncateText(responseText),
    };

    if (!response.ok) {
      return {
        success: false,
        retryable: response.status >= 500 || response.status === 429,
        reason: pushdeerT("errors.requestReturned", {
          status: response.status,
        }),
        request_summary: requestSummary,
        response_summary: responseSummary,
      };
    }

    if ((parsedResponse?.code ?? 0) !== 0) {
      return {
        success: false,
        retryable: false,
        reason:
          parsedResponse?.error ||
          pushdeerT("errors.apiReturnedCode", {
            code: String(parsedResponse?.code ?? "unknown"),
          }),
        request_summary: requestSummary,
        response_summary: responseSummary,
      };
    }

    return {
      success: true,
      retryable: false,
      request_summary: requestSummary,
      response_summary: responseSummary,
    };
  } catch (error) {
    return {
      success: false,
      retryable: true,
      reason:
        error instanceof Error
          ? error.message
          : pushdeerT("errors.requestFailed"),
      request_summary: {
        method: "POST",
        url,
        type: "markdown",
      },
    };
  } finally {
    clearTimeout(timeout);
  }
};
