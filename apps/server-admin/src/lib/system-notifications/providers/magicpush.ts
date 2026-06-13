import type {
  NotificationMessage,
  NotificationProvider,
  NotificationProviderDefinition,
  NotificationSchemaField,
  NotificationSendResult,
} from "../types";
import { toPlainRecord, toTrimmedString, truncateText } from "./shared";

const MAGICPUSH_CONNECTION_SCHEMA: NotificationSchemaField[] = [
  {
    key: "server_url",
    label: "基础 API 地址",
    description:
      "填写 MagicPush 服务根地址，例如 http://192.168.31.98:3000；如果已填写到 /api/push 或 /api/inbound 也会直接使用。",
    placeholder: "http://192.168.31.98:3000",
    type: "string",
    required: true,
  },
  {
    key: "delivery_mode",
    label: "投递模式",
    description:
      "标准推送会发送到 /api/push；入站配置会发送到 /api/inbound/:token，由 MagicPush 的入站规则负责字段映射。",
    type: "select",
    required: true,
    default_value: "push",
    options: [
      { label: "标准推送", value: "push" },
      { label: "入站配置", value: "inbound" },
    ],
  },
  {
    key: "token",
    label: "Token",
    description:
      "MagicPush 接口令牌。标准推送会通过 Authorization: Bearer 发送；入站配置会拼接到 /api/inbound/:token。",
    placeholder: "your_token",
    type: "string",
    required: true,
  },
  {
    key: "timeout_seconds",
    label: "超时秒数",
    type: "number",
    required: true,
    default_value: 5,
    min: 1,
    max: 30,
  },
];

const MAGICPUSH_TARGET_SCHEMA: NotificationSchemaField[] = [];

export const magicpushProviderDefinition: NotificationProviderDefinition = {
  type: "magicpush",
  label: "MagicPush魔法推送",
  description:
    "通过 MagicPush 自建服务向已配置的渠道推送通知，支持标准推送和 MagicPush 入站配置。",
  connection_schema: MAGICPUSH_CONNECTION_SCHEMA,
  target_schema: MAGICPUSH_TARGET_SCHEMA,
  sensitive_fields: [],
  capabilities: {
    supports_text: true,
    supports_markdown: false,
    supports_rich_blocks: false,
    supports_actions: false,
    supports_mentions: false,
    supports_attachments: false,
    supports_provider_dedupe_key: false,
    max_body_length: null,
  },
};

const resolveMagicPushUrl = (
  provider: NotificationProvider,
  token: string,
  deliveryMode: "push" | "inbound",
) => {
  const baseUrl = toTrimmedString(provider.connection_config.server_url);
  if (!baseUrl) {
    throw new Error("Missing MagicPush base API url");
  }

  const normalizedBaseUrl = baseUrl.replace(/\/+$/, "");
  if (deliveryMode === "inbound") {
    if (/\/api\/inbound\/[^/]+$/i.test(normalizedBaseUrl)) {
      return normalizedBaseUrl;
    }
    if (/\/api\/inbound$/i.test(normalizedBaseUrl)) {
      return `${normalizedBaseUrl}/${encodeURIComponent(token)}`;
    }
    return `${normalizedBaseUrl}/api/inbound/${encodeURIComponent(token)}`;
  }

  if (/\/api\/push(?:\/[^/]+)?$/i.test(normalizedBaseUrl)) {
    return normalizedBaseUrl;
  }

  return `${normalizedBaseUrl}/api/push`;
};

const buildMagicPushContent = (message: NotificationMessage) => {
  const sections: string[] = [];

  if (message.summary?.trim()) {
    sections.push(message.summary.trim());
  }

  if (message.body_text?.trim()) {
    sections.push(message.body_text.trim());
  }

  if (message.facts.length > 0) {
    sections.push(
      message.facts.map((fact) => `${fact.label}: ${fact.value}`).join("\n"),
    );
  }

  if (message.actions.length > 0) {
    sections.push(
      message.actions
        .filter(
          (action) =>
            toTrimmedString(action.label) && toTrimmedString(action.url),
        )
        .map((action) => `${action.label}: ${action.url}`)
        .join("\n"),
    );
  }

  return sections.filter(Boolean).join("\n\n");
};

const parseMagicPushApiCode = (response: Record<string, unknown> | null) => {
  const rawCode = response?.code;
  if (typeof rawCode === "number") {
    return rawCode;
  }

  const parsed = Number.parseInt(String(rawCode ?? ""), 10);
  return Number.isFinite(parsed) ? parsed : undefined;
};

const buildMagicPushInboundPayload = (message: NotificationMessage) => {
  const content = buildMagicPushContent(message) || message.title;
  const facts = message.facts.reduce<Record<string, string>>((acc, fact) => {
    const label = toTrimmedString(fact.label);
    if (!label) return acc;
    acc[label] = fact.value;
    return acc;
  }, {});

  return {
    source: "fn-knock",
    title: toTrimmedString(message.title || message.summary || "fn-knock 通知"),
    summary: message.summary,
    content,
    body: content,
    body_text: message.body_text,
    body_markdown: message.body_markdown,
    type: message.body_markdown ? "markdown" : "text",
    severity: message.severity,
    facts,
    facts_list: message.facts,
    actions: message.actions,
    mentions: message.mentions,
    dedupe_key: message.dedupe_key,
    occurred_at: message.occurred_at,
    event_id: message.event_id,
    metadata: message.metadata,
  };
};

export const sendMagicPushMessage = async (args: {
  provider: NotificationProvider;
  message: NotificationMessage;
  timeoutSeconds: number;
}): Promise<NotificationSendResult> => {
  const token = toTrimmedString(args.provider.connection_config.token);
  if (!token) {
    return {
      success: false,
      retryable: false,
      reason: "Missing MagicPush token",
    };
  }

  const deliveryMode =
    toTrimmedString(args.provider.connection_config.delivery_mode) === "inbound"
      ? "inbound"
      : "push";
  let url: string;
  try {
    url = resolveMagicPushUrl(args.provider, token, deliveryMode);
  } catch (error) {
    return {
      success: false,
      retryable: false,
      reason:
        error instanceof Error ? error.message : "Invalid MagicPush base url",
    };
  }

  const title = toTrimmedString(
    args.message.title || args.message.summary || "fn-knock 通知",
  );
  const content = buildMagicPushContent(args.message) || title;
  const payload =
    deliveryMode === "inbound"
      ? buildMagicPushInboundPayload(args.message)
      : {
          title,
          content,
          type: "text",
        };
  const headers: Record<string, string> =
    deliveryMode === "inbound"
      ? {
          "content-type": "application/json; charset=utf-8",
          "x-fn-knock-provider": "magicpush",
        }
      : {
          authorization: `Bearer ${token}`,
          "content-type": "application/json; charset=utf-8",
        };

  const controller = new AbortController();
  const timeout = setTimeout(
    () => controller.abort(),
    Math.max(1, args.timeoutSeconds) * 1000,
  );

  try {
    const response = await fetch(url, {
      method: "POST",
      headers,
      body: JSON.stringify(payload),
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

    const apiCode = parseMagicPushApiCode(parsedResponse);
    const apiMessage = toTrimmedString(
      parsedResponse?.message ?? parsedResponse?.msg ?? parsedResponse?.error,
    );
    const data = toPlainRecord(parsedResponse?.data);
    const apiSuccess = parsedResponse?.success;
    const succeeded =
      response.ok &&
      apiSuccess !== false &&
      (apiCode === undefined || apiCode === 200);

    return {
      success: succeeded,
      retryable:
        !succeeded && (response.status >= 500 || response.status === 429),
      reason: succeeded
        ? undefined
        : apiMessage || `MagicPush returned ${response.status}`,
      request_summary: {
        method: "POST",
        url,
        delivery_mode: deliveryMode,
        type: payload.type,
        title_preview: payload.title,
        content_preview: truncateText(payload.content),
      },
      response_summary: {
        status: response.status,
        ok: response.ok,
        success: apiSuccess,
        code: apiCode,
        message: apiMessage || undefined,
        total: data.total,
        success_count: data.successCount,
        failed_count: data.failedCount,
        body_preview: truncateText(responseText),
      },
    };
  } catch (error) {
    return {
      success: false,
      retryable: true,
      reason:
        error instanceof Error ? error.message : "MagicPush request failed",
      request_summary: {
        method: "POST",
        url,
        delivery_mode: deliveryMode,
        type: payload.type,
        title_preview: payload.title,
      },
    };
  } finally {
    clearTimeout(timeout);
  }
};
