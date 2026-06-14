import {
  getRegisteredNotificationProvider,
  listRegisteredNotificationProviders,
} from "./providers";
import { normalizeNotificationMessage } from "./brand";
import { tWithFallback } from "../i18n";
import {
  DEFAULT_LOCALE,
  type LocaleCode,
} from "../../../../../packages/i18n/src";
import type {
  NotificationDispatchContext,
  NotificationMessage,
  NotificationProvider,
  NotificationProviderDetailView,
  NotificationProviderDefinition,
  NotificationSchemaField,
  NotificationProviderView,
  NotificationSendResult,
} from "./types";

const maskSensitiveValue = (value: unknown) => {
  if (value === undefined || value === null || value === "") return "";
  if (typeof value === "string") {
    return value.length <= 8 ? "********" : `${value.slice(0, 2)}******`;
  }
  return "[configured]";
};

const localizeSchemaField = (
  locale: LocaleCode,
  providerType: string,
  field: NotificationSchemaField,
  scope: "connection" | "target",
): NotificationSchemaField => {
  const baseKey = `server.notifications.providers.catalog.${providerType}.fields.${field.key}`;
  const labelKey =
    scope === "target" ? `${baseKey}.targetLabel` : `${baseKey}.label`;
  const descriptionKey =
    scope === "target"
      ? `${baseKey}.targetDescription`
      : `${baseKey}.description`;
  const placeholderKey =
    scope === "target"
      ? `${baseKey}.targetPlaceholder`
      : `${baseKey}.placeholder`;
  return {
    ...field,
    label: tWithFallback(
      locale,
      labelKey,
      tWithFallback(locale, `${baseKey}.label`, field.label),
    ),
    ...(field.description
      ? {
          description: tWithFallback(
            locale,
            descriptionKey,
            tWithFallback(locale, `${baseKey}.description`, field.description),
          ),
        }
      : {}),
    ...(field.placeholder
      ? {
          placeholder: tWithFallback(
            locale,
            placeholderKey,
            tWithFallback(locale, `${baseKey}.placeholder`, field.placeholder),
          ),
        }
      : {}),
    ...(field.options
      ? {
          options: field.options.map((option) => ({
            ...option,
            label: tWithFallback(
              locale,
              `${baseKey}.options.${option.value}`,
              option.label,
            ),
          })),
        }
      : {}),
  };
};

const localizeProviderDefinition = (
  definition: NotificationProviderDefinition,
  locale: LocaleCode,
): NotificationProviderDefinition => {
  const baseKey = `server.notifications.providers.catalog.${definition.type}`;
  return {
    ...definition,
    label: tWithFallback(locale, `${baseKey}.label`, definition.label),
    description: tWithFallback(
      locale,
      `${baseKey}.description`,
      definition.description,
    ),
    connection_schema: definition.connection_schema.map((field) =>
      localizeSchemaField(locale, definition.type, field, "connection"),
    ),
    target_schema: definition.target_schema.map((field) =>
      localizeSchemaField(locale, definition.type, field, "target"),
    ),
  };
};

export const listNotificationProviderDefinitions = (
  locale: LocaleCode = DEFAULT_LOCALE,
) =>
  listRegisteredNotificationProviders().map((registration) =>
    localizeProviderDefinition(registration.definition, locale),
  );

export const getNotificationProviderDefinition = (
  type: string,
): NotificationProviderDefinition | null =>
  getRegisteredNotificationProvider(type)?.definition || null;

export const maskNotificationProvider = (
  provider: NotificationProvider,
): NotificationProviderView => {
  const definition = getNotificationProviderDefinition(provider.type);
  const masked: Record<string, unknown> = {};

  for (const [key, value] of Object.entries(provider.connection_config)) {
    masked[key] = definition?.sensitive_fields.includes(key)
      ? maskSensitiveValue(value)
      : value;
  }

  return {
    id: provider.id,
    name: provider.name,
    type: provider.type,
    enabled: provider.enabled,
    created_at: provider.created_at,
    updated_at: provider.updated_at,
    last_test_at: provider.last_test_at,
    last_test_status: provider.last_test_status,
    last_error: provider.last_error,
    connection_config_masked: masked,
  };
};

export const revealNotificationProvider = (
  provider: NotificationProvider,
): NotificationProviderDetailView => ({
  ...maskNotificationProvider(provider),
  connection_config: {
    ...provider.connection_config,
  },
});

export const sendNotificationWithProvider = async (
  provider: NotificationProvider,
  message: NotificationMessage,
  context?: Partial<NotificationDispatchContext>,
  timeoutSeconds = 5,
  locale?: string | null,
): Promise<NotificationSendResult> => {
  const registration = getRegisteredNotificationProvider(provider.type);
  if (!registration) {
    return {
      success: false,
      retryable: false,
      reason: `Unsupported notification provider type: ${provider.type}`,
    };
  }

  const normalizedMessage = normalizeNotificationMessage(message, locale);

  return registration.send({
    provider,
    message: normalizedMessage,
    context,
    timeoutSeconds,
  });
};
