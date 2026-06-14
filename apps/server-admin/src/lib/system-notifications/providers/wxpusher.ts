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
  resolveOptionalNonNegativeInteger,
  resolvePrimaryActionUrl,
  splitCommaSeparatedValues,
  toPlainRecord,
  toTrimmedString,
  truncateText,
  truncateUtf8ByBytes,
} from "./shared";
import { tDefault } from "../../i18n";

const WXPUSHER_INHERIT_TARGET_VALUE = "__inherit__";
const wxpusherT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) =>
  tDefault(`server.notifications.providers.catalog.wxpusher.${key}`, params);

const WXPUSHER_UIDS_FIELD: NotificationSchemaField = {
  key: "uids",
  label: wxpusherT("fields.uids.targetLabel"),
  description: wxpusherT("fields.uids.description"),
  placeholder: "UID_xxx,UID_yyy",
  type: "string",
};

const WXPUSHER_TOPIC_IDS_FIELD: NotificationSchemaField = {
  key: "topic_ids",
  label: "Topic",
  description: wxpusherT("fields.topic_ids.description"),
  placeholder: "123,456",
  type: "string",
};

const WXPUSHER_URL_FIELD: NotificationSchemaField = {
  key: "url",
  label: wxpusherT("fields.url.targetLabel"),
  description: wxpusherT("fields.url.description"),
  placeholder: "https://example.com/events/123",
  type: "string",
};

const WXPUSHER_VERIFY_PAY_TYPE_FIELD: NotificationSchemaField = {
  key: "verify_pay_type",
  label: wxpusherT("fields.verify_pay_type.targetLabel"),
  description: wxpusherT("fields.verify_pay_type.description"),
  type: "select",
  default_value: "0",
  options: [
    { label: wxpusherT("fields.verify_pay_type.options.0"), value: "0" },
    { label: wxpusherT("fields.verify_pay_type.options.1"), value: "1" },
    { label: wxpusherT("fields.verify_pay_type.options.2"), value: "2" },
  ],
};

const WXPUSHER_CONNECTION_SCHEMA: NotificationSchemaField[] = [
  {
    key: "server_url",
    label: wxpusherT("fields.server_url.label"),
    description: wxpusherT("fields.server_url.description"),
    placeholder: "https://wxpusher.zjiecode.com",
    type: "string",
    required: true,
    default_value: "https://wxpusher.zjiecode.com",
  },
  {
    key: "app_token",
    label: "AppToken",
    description: wxpusherT("fields.app_token.description"),
    placeholder: "AT_xxx",
    type: "string",
    required: true,
    sensitive: true,
  },
  {
    key: "timeout_seconds",
    label: wxpusherT("fields.timeout_seconds.label"),
    type: "number",
    required: true,
    default_value: 5,
    min: 1,
    max: 30,
  },
  {
    ...WXPUSHER_UIDS_FIELD,
    label: wxpusherT("fields.uids.label"),
    description: wxpusherT("fields.uids.description"),
  },
  {
    ...WXPUSHER_TOPIC_IDS_FIELD,
    label: wxpusherT("fields.topic_ids.label"),
    description: wxpusherT("fields.topic_ids.description"),
  },
  {
    ...WXPUSHER_URL_FIELD,
    label: wxpusherT("fields.url.label"),
    description: wxpusherT("fields.url.description"),
  },
  {
    ...WXPUSHER_VERIFY_PAY_TYPE_FIELD,
    label: wxpusherT("fields.verify_pay_type.label"),
    description: wxpusherT("fields.verify_pay_type.description"),
  },
];

const WXPUSHER_TARGET_SCHEMA: NotificationSchemaField[] = [
  {
    ...WXPUSHER_UIDS_FIELD,
    description: wxpusherT("fields.uids.targetDescription"),
  },
  {
    ...WXPUSHER_TOPIC_IDS_FIELD,
    description: wxpusherT("fields.topic_ids.targetDescription"),
  },
  {
    ...WXPUSHER_URL_FIELD,
    description: wxpusherT("fields.url.targetDescription"),
  },
  {
    ...WXPUSHER_VERIFY_PAY_TYPE_FIELD,
    description: wxpusherT("fields.verify_pay_type.targetDescription"),
    default_value: WXPUSHER_INHERIT_TARGET_VALUE,
    options: [
      {
        label: wxpusherT("fields.verify_pay_type.options.__inherit__"),
        value: WXPUSHER_INHERIT_TARGET_VALUE,
      },
      ...(WXPUSHER_VERIFY_PAY_TYPE_FIELD.options || []),
    ],
  },
];

export const wxpusherProviderDefinition: NotificationProviderDefinition = {
  type: "wxpusher",
  label: "WxPusher",
  description: wxpusherT("description"),
  connection_schema: WXPUSHER_CONNECTION_SCHEMA,
  target_schema: WXPUSHER_TARGET_SCHEMA,
  sensitive_fields: ["app_token"],
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

const resolveWxPusherUrl = (provider: NotificationProvider) => {
  const baseUrl = toTrimmedString(
    provider.connection_config.server_url ??
      provider.connection_config.serverUrl,
  );
  return `${(baseUrl || "https://wxpusher.zjiecode.com").replace(/\/+$/, "")}/api/send/message`;
};

const parseTopicIds = (value: unknown) => {
  const rawValues = splitCommaSeparatedValues(value);
  const topicIds: number[] = [];
  const invalidValues: string[] = [];

  for (const rawValue of rawValues) {
    if (!/^\d+$/.test(rawValue)) {
      invalidValues.push(rawValue);
      continue;
    }

    const parsed = Number.parseInt(rawValue, 10);
    if (!Number.isFinite(parsed) || parsed <= 0) {
      invalidValues.push(rawValue);
      continue;
    }

    topicIds.push(parsed);
  }

  return {
    topicIds,
    invalidValues,
  };
};

const resolveFirstConfiguredValue = (
  config: Record<string, unknown>,
  keys: string[],
) => {
  for (const key of keys) {
    if (!(key in config)) continue;
    const value = config[key];
    if (value === undefined || value === null) continue;
    if (typeof value === "string" && !value.trim()) continue;
    return value;
  }
  return undefined;
};

const resolveEffectiveWxPusherValue = (
  providerConfig: Record<string, unknown>,
  targetConfig: Record<string, unknown>,
  keys: string[],
) => {
  const targetValue = resolveFirstConfiguredValue(targetConfig, keys);
  if (
    targetValue !== undefined &&
    !(
      typeof targetValue === "string" &&
      targetValue === WXPUSHER_INHERIT_TARGET_VALUE
    )
  ) {
    return targetValue;
  }

  return resolveFirstConfiguredValue(providerConfig, keys);
};

const buildWxPusherHtmlContent = (message: NotificationMessage) => {
  const sections: string[] = [];
  const title = toTrimmedString(
    message.title || wxpusherT("message.fallbackTitle"),
  );
  const summary = toTrimmedString(message.summary);
  const bodyText = toTrimmedString(message.body_text);

  if (title) {
    sections.push(`<h2>${escapeHtml(title)}</h2>`);
  }
  if (summary) {
    sections.push(`<p>${escapeHtml(summary)}</p>`);
  }
  if (bodyText) {
    const bodyHtml = bodyText
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => `<p>${escapeHtml(line)}</p>`)
      .join("");
    if (bodyHtml) {
      sections.push(bodyHtml);
    }
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
      message.actions
        .filter(
          (action) =>
            toTrimmedString(action.label) && toTrimmedString(action.url),
        )
        .map(
          (action) =>
            `<p><a href="${escapeHtml(action.url.trim())}">${escapeHtml(action.label.trim())}</a></p>`,
        )
        .join(""),
    );
  }

  return sections.filter(Boolean).join("");
};

export const sendWxPusherMessage = async (args: {
  provider: NotificationProvider;
  message: NotificationMessage;
  context?: Partial<NotificationDispatchContext>;
  timeoutSeconds: number;
}): Promise<NotificationSendResult> => {
  const appToken = toTrimmedString(
    resolveFirstConfiguredValue(args.provider.connection_config, [
      "app_token",
      "appToken",
    ]),
  );
  if (!appToken) {
    return {
      success: false,
      retryable: false,
      reason: wxpusherT("errors.missingAppToken"),
    };
  }

  const providerConfig = toPlainRecord(args.provider.connection_config);
  const targetConfig = toPlainRecord(args.context?.target?.target_config);
  const uids = splitCommaSeparatedValues(
    resolveEffectiveWxPusherValue(providerConfig, targetConfig, ["uids"]),
  );
  const { topicIds, invalidValues } = parseTopicIds(
    resolveEffectiveWxPusherValue(providerConfig, targetConfig, [
      "topic_ids",
      "topicIds",
      "topic_id",
      "topicId",
      "topic",
      "Topic",
    ]),
  );

  if (invalidValues.length > 0) {
    return {
      success: false,
      retryable: false,
      reason: wxpusherT("errors.invalidTopicIds", {
        values: invalidValues.join(", "),
      }),
    };
  }

  if (uids.length === 0 && topicIds.length === 0) {
    return {
      success: false,
      retryable: false,
      reason: wxpusherT("errors.recipientRequired"),
    };
  }

  const url =
    toTrimmedString(
      resolveEffectiveWxPusherValue(providerConfig, targetConfig, ["url"]),
    ) || resolvePrimaryActionUrl(args.message);
  const verifyPayType = resolveOptionalNonNegativeInteger(
    resolveEffectiveWxPusherValue(providerConfig, targetConfig, [
      "verify_pay_type",
      "verifyPayType",
    ]),
  );
  const requestBody = {
    appToken,
    content:
      buildWxPusherHtmlContent(args.message) ||
      `<p>${wxpusherT("message.fallbackTitle")}</p>`,
    summary: truncateUtf8ByBytes(
      toTrimmedString(
        args.message.summary ||
          args.message.title ||
          wxpusherT("message.fallbackTitle"),
      ),
      100,
    ),
    contentType: 2,
    ...(topicIds.length > 0 ? { topicIds } : {}),
    ...(uids.length > 0 ? { uids } : {}),
    ...(url ? { url } : {}),
    ...(verifyPayType !== undefined && verifyPayType <= 2
      ? { verifyPayType }
      : {}),
  };
  const urlToSend = resolveWxPusherUrl(args.provider);

  const controller = new AbortController();
  const timeout = setTimeout(
    () => controller.abort(),
    Math.max(1, args.timeoutSeconds) * 1000,
  );

  try {
    const response = await fetch(urlToSend, {
      method: "POST",
      headers: {
        "content-type": "application/json; charset=utf-8",
      },
      body: JSON.stringify(requestBody),
      signal: controller.signal,
    });
    const responseText = await response.text().catch(() => "");
    let parsedResponse: {
      code?: number;
      msg?: string;
      success?: boolean;
      data?: Array<{
        uid?: string;
        topicId?: number | null;
        sendRecordId?: number;
        messageContentId?: number;
        code?: number;
        status?: string;
      }>;
    } | null = null;
    try {
      parsedResponse = responseText ? JSON.parse(responseText) : null;
    } catch {
      parsedResponse = null;
    }

    const itemResults = parsedResponse?.data || [];
    const failedItems = itemResults.filter((item) => item.code !== 1000);
    const apiSucceeded =
      response.ok &&
      (parsedResponse?.success ?? true) &&
      (parsedResponse?.code ?? 1000) === 1000 &&
      failedItems.length === 0;

    return {
      success: apiSucceeded,
      retryable:
        !apiSucceeded && (response.status >= 500 || response.status === 429),
      reason: apiSucceeded
        ? undefined
        : failedItems.length > 0
          ? wxpusherT("errors.targetsFailed", {
              failed: failedItems.length,
              total: itemResults.length,
            })
          : parsedResponse?.msg || `WxPusher returned ${response.status}`,
      request_summary: {
        method: "POST",
        url: urlToSend,
        uid_count: uids.length,
        topic_id_count: topicIds.length,
        content_type: 2,
        has_url: Boolean(url),
      },
      response_summary: {
        status: response.status,
        ok: response.ok,
        code: parsedResponse?.code,
        success: parsedResponse?.success,
        msg: parsedResponse?.msg,
        success_count: itemResults.length - failedItems.length,
        failed_count: failedItems.length,
        items: itemResults.map((item) => ({
          uid: item.uid,
          topic_id: item.topicId,
          send_record_id: item.sendRecordId,
          message_content_id: item.messageContentId,
          code: item.code,
          status: item.status,
        })),
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
          : wxpusherT("errors.requestFailed"),
      request_summary: {
        method: "POST",
        url: urlToSend,
        uid_count: uids.length,
        topic_id_count: topicIds.length,
        content_type: 2,
      },
    };
  } finally {
    clearTimeout(timeout);
  }
};
