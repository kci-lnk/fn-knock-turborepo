import type {
  NotificationDispatchContext,
  NotificationMessage,
  NotificationProvider,
  NotificationProviderDefinition,
  NotificationSchemaField,
  NotificationSendResult,
} from "../types";
import {
  resolveOptionalNonNegativeInteger,
  resolvePrimaryActionUrl,
  splitCommaSeparatedValues,
  toPlainRecord,
  toTrimmedString,
  truncateText,
} from "./shared";
import { tDefault } from "../../i18n";

const barkT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.notifications.providers.catalog.bark.${key}`, params);

const BARK_CONNECTION_SCHEMA: NotificationSchemaField[] = [
  {
    key: "server_url",
    label: barkT("fields.server_url.label"),
    description: barkT("fields.server_url.description"),
    placeholder: "https://api.day.app",
    type: "string",
    required: true,
    default_value: "https://api.day.app",
  },
  {
    key: "device_key",
    label: "Device Key",
    description: barkT("fields.device_key.description"),
    placeholder: "ynJ5Ft4atkMkWeo2PAvFhF",
    type: "string",
    required: true,
    sensitive: true,
  },
  {
    key: "timeout_seconds",
    label: barkT("fields.timeout_seconds.label"),
    type: "number",
    required: true,
    default_value: 5,
    min: 1,
    max: 30,
  },
];

const BARK_TARGET_SCHEMA: NotificationSchemaField[] = [
  {
    key: "level",
    label: barkT("fields.level.label"),
    description: barkT("fields.level.description"),
    type: "select",
    default_value: "active",
    options: [
      { label: "active", value: "active" },
      { label: "timeSensitive", value: "timeSensitive" },
      { label: "passive", value: "passive" },
      { label: "critical", value: "critical" },
    ],
  },
  {
    key: "group",
    label: barkT("fields.group.label"),
    description: barkT("fields.group.description"),
    placeholder: "fn-knock",
    type: "string",
  },
  {
    key: "sound",
    label: barkT("fields.sound.label"),
    description: barkT("fields.sound.description"),
    placeholder: "alarm",
    type: "string",
  },
  {
    key: "url",
    label: barkT("fields.url.label"),
    description: barkT("fields.url.description"),
    placeholder: "https://example.com/events/123",
    type: "string",
  },
  {
    key: "icon",
    label: barkT("fields.icon.label"),
    description: barkT("fields.icon.description"),
    placeholder: "https://day.app/assets/images/avatar.jpg",
    type: "string",
  },
  {
    key: "badge",
    label: barkT("fields.badge.label"),
    description: barkT("fields.badge.description"),
    type: "number",
    min: 0,
    max: 99999,
  },
  {
    key: "call",
    label: barkT("fields.call.label"),
    description: barkT("fields.call.description"),
    type: "boolean",
    default_value: false,
  },
];

export const barkProviderDefinition: NotificationProviderDefinition = {
  type: "bark",
  label: "Bark",
  description: barkT("description"),
  connection_schema: BARK_CONNECTION_SCHEMA,
  target_schema: BARK_TARGET_SCHEMA,
  sensitive_fields: ["device_key"],
  capabilities: {
    supports_text: true,
    supports_markdown: false,
    supports_rich_blocks: false,
    supports_actions: true,
    supports_mentions: false,
    supports_attachments: false,
    supports_provider_dedupe_key: false,
    max_body_length: null,
  },
};

const resolveBarkUrl = (provider: NotificationProvider) => {
  const baseUrl = toTrimmedString(provider.connection_config.server_url);
  const normalizedBaseUrl = baseUrl || "https://api.day.app";
  return `${normalizedBaseUrl.replace(/\/+$/, "")}/push`;
};

const buildBarkPayload = (
  message: NotificationMessage,
  context?: Partial<NotificationDispatchContext>,
) => {
  const targetConfig = toPlainRecord(context?.target?.target_config);
  const summary = toTrimmedString(message.summary);
  const bodyText = toTrimmedString(message.body_text);
  const hasStandaloneBody = Boolean(bodyText) && bodyText !== summary;
  const title = toTrimmedString(message.title || barkT("message.fallbackTitle"));
  const subtitle = hasStandaloneBody ? summary : "";
  const body = hasStandaloneBody ? bodyText : summary || bodyText || title;
  const url =
    toTrimmedString(targetConfig.url) || resolvePrimaryActionUrl(message);
  const level = toTrimmedString(targetConfig.level || "active");
  const sound = toTrimmedString(targetConfig.sound);
  const group = toTrimmedString(targetConfig.group);
  const icon = toTrimmedString(targetConfig.icon);
  const badge = resolveOptionalNonNegativeInteger(targetConfig.badge);
  const call = Boolean(targetConfig.call);

  return {
    title,
    subtitle: subtitle || undefined,
    body: body || barkT("message.fallbackTitle"),
    level: level || "active",
    ...(sound ? { sound } : {}),
    ...(group ? { group } : {}),
    ...(url ? { url } : {}),
    ...(icon ? { icon } : {}),
    ...(badge !== undefined ? { badge } : {}),
    ...(call ? { call: "1" } : {}),
  };
};

const sendSingleBarkPush = async (args: {
  url: string;
  deviceKey: string;
  message: NotificationMessage;
  context?: Partial<NotificationDispatchContext>;
  timeoutSeconds: number;
}) => {
  const payload = {
    ...buildBarkPayload(args.message, args.context),
    device_key: args.deviceKey,
  };
  const controller = new AbortController();
  const timeout = setTimeout(
    () => controller.abort(),
    Math.max(1, args.timeoutSeconds) * 1000,
  );

  try {
    const response = await fetch(args.url, {
      method: "POST",
      headers: {
        "content-type": "application/json; charset=utf-8",
      },
      body: JSON.stringify(payload),
      signal: controller.signal,
    });
    const responseText = await response.text().catch(() => "");
    let parsedResponse: {
      code?: number;
      message?: string;
      timestamp?: number;
    } | null = null;
    try {
      parsedResponse = responseText ? JSON.parse(responseText) : null;
    } catch {
      parsedResponse = null;
    }

    const barkCode = parsedResponse?.code;
    const succeeded =
      response.ok && (barkCode === undefined || barkCode === 200);
    const reason =
      parsedResponse?.message ||
      (response.ok ? "" : `Bark returned ${response.status}`);

    return {
      success: succeeded,
      retryable:
        !succeeded && (response.status >= 500 || response.status === 429),
      reason: succeeded ? undefined : reason,
      response_summary: {
        status: response.status,
        ok: response.ok,
        code: barkCode,
        message: parsedResponse?.message,
        body_preview: truncateText(responseText),
      },
    };
  } catch (error) {
    return {
      success: false,
      retryable: true,
      reason:
        error instanceof Error ? error.message : barkT("errors.requestFailed"),
      response_summary: null,
    };
  } finally {
    clearTimeout(timeout);
  }
};

export const sendBarkMessage = async (args: {
  provider: NotificationProvider;
  message: NotificationMessage;
  context?: Partial<NotificationDispatchContext>;
  timeoutSeconds: number;
}): Promise<NotificationSendResult> => {
  const url = resolveBarkUrl(args.provider);
  const deviceKeys = splitCommaSeparatedValues(
    args.provider.connection_config.device_key,
  );
  if (!deviceKeys.length) {
    return {
      success: false,
      retryable: false,
      reason: barkT("errors.missingDeviceKey"),
    };
  }

  const payloadPreview = buildBarkPayload(args.message, args.context);
  const results = await Promise.all(
    deviceKeys.map((deviceKey) =>
      sendSingleBarkPush({
        url,
        deviceKey,
        message: args.message,
        context: args.context,
        timeoutSeconds: args.timeoutSeconds,
      }),
    ),
  );

  const failedResults = results.filter((result) => !result.success);
  if (!failedResults.length) {
    return {
      success: true,
      retryable: false,
      request_summary: {
        method: "POST",
        url,
        device_key_count: deviceKeys.length,
        level: payloadPreview.level,
        group: payloadPreview.group,
        title_preview: payloadPreview.title,
      },
      response_summary: {
        success_count: results.length,
        failed_count: 0,
        results: results.map((result) => result.response_summary),
      },
    };
  }

  return {
    success: false,
    retryable: failedResults.some((result) => result.retryable),
    reason:
      failedResults.length === 1
        ? failedResults[0]!.reason || barkT("errors.pushFailed")
        : barkT("errors.targetsFailed", {
            failed: failedResults.length,
            total: results.length,
          }),
    request_summary: {
      method: "POST",
      url,
      device_key_count: deviceKeys.length,
      level: payloadPreview.level,
      group: payloadPreview.group,
      title_preview: payloadPreview.title,
    },
    response_summary: {
      success_count: results.length - failedResults.length,
      failed_count: failedResults.length,
      results: results.map((result) => ({
        success: result.success,
        retryable: result.retryable,
        reason: result.reason,
        response_summary: result.response_summary,
      })),
    },
  };
};
