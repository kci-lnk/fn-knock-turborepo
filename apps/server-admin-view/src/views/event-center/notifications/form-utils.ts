import type {
  NotificationProviderType,
  NotificationSchemaField,
} from "../../../types";
import {
  coerceWebhookHeaderEntries,
  normalizeWebhookHeaderEntries,
} from "./webhook-headers";
import {
  coerceWebhookBodyConfig,
  normalizeWebhookBodyConfig,
  type WebhookBodyConstraints,
  type WebhookBodyScope,
} from "./webhook-body";

const webhookBodyScope = (field: NotificationSchemaField): WebhookBodyScope =>
  (field.constraints as WebhookBodyConstraints | undefined)?.scope === "target"
    ? "target"
    : "provider";

export type ProviderDialogMode = "create" | "edit";

export type EditableProviderForm = {
  name: string;
  type: NotificationProviderType | "";
  enabled: boolean;
  connection_config: Record<string, unknown>;
};

export type ProviderFormPayload = {
  name?: string;
  type: NotificationProviderType;
  enabled: boolean;
  connection_config: Record<string, unknown>;
};

const escapeRegExp = (value: string) =>
  value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

export const createEditableSchemaRecord = (
  fields: NotificationSchemaField[],
  source: Record<string, unknown> = {},
) =>
  fields.reduce<Record<string, unknown>>((acc, field) => {
    const value = source[field.key];
    if (field.type === "headers") {
      acc[field.key] = coerceWebhookHeaderEntries(value);
      return acc;
    }
    if (field.type === "webhook_body") {
      acc[field.key] = coerceWebhookBodyConfig(value, webhookBodyScope(field));
      return acc;
    }
    if (value === undefined || value === null) {
      acc[field.key] =
        field.type === "boolean"
          ? Boolean(field.default_value ?? false)
          : (field.default_value ?? "");
      return acc;
    }

    if (field.type === "json" && typeof value === "object") {
      acc[field.key] = JSON.stringify(value, null, 2);
      return acc;
    }

    acc[field.key] = value;
    return acc;
  }, {});

export const buildSchemaPayload = (args: {
  fields: NotificationSchemaField[];
  value: Record<string, unknown>;
  editing?: boolean;
  configuredSensitiveFields?: string[];
}) => {
  const configuredSensitiveFields = new Set(
    args.configuredSensitiveFields || [],
  );
  const payload: Record<string, unknown> = {};

  for (const field of args.fields) {
    const raw = args.value[field.key];
    if (field.type === "headers") {
      payload[field.key] = normalizeWebhookHeaderEntries(raw);
      continue;
    }
    if (field.type === "webhook_body") {
      payload[field.key] = normalizeWebhookBodyConfig(
        raw,
        webhookBodyScope(field),
      );
      continue;
    }
    if (field.sensitive) {
      const text = String(raw ?? "").trim();
      if (args.editing && configuredSensitiveFields.has(field.key) && !text) {
        continue;
      }
      if (!text) {
        if (field.required) {
          payload[field.key] = "";
        }
        continue;
      }
      payload[field.key] = text;
      continue;
    }

    if (typeof raw === "string") {
      const trimmed = raw.trim();
      if (!trimmed) {
        if (field.required) {
          payload[field.key] = "";
        }
        continue;
      }
      payload[field.key] = trimmed;
      continue;
    }

    if (raw === undefined || raw === null) {
      if (field.required) {
        payload[field.key] = "";
      }
      continue;
    }

    payload[field.key] = raw;
  }

  return payload;
};

export const buildNextSequentialName = (
  baseLabel: string,
  existingNames: string[],
  fallbackLabel = "Untitled",
) => {
  const normalizedBase = baseLabel.trim() || fallbackLabel;
  const pattern = new RegExp(`^${escapeRegExp(normalizedBase)}\\s+(\\d+)$`);
  const usedIndexes = new Set<number>();

  for (const name of existingNames) {
    const match = name.trim().match(pattern);
    if (!match) continue;

    const index = Number.parseInt(match[1] || "", 10);
    if (Number.isFinite(index) && index > 0) {
      usedIndexes.add(index);
    }
  }

  let nextIndex = 1;
  while (usedIndexes.has(nextIndex)) {
    nextIndex += 1;
  }

  return `${normalizedBase} ${nextIndex}`;
};
