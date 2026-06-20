import { parseNumberField, serviceT } from "./common";

type SchemaField = {
  key: string;
  label: string;
  type: "string" | "number" | "boolean" | "select" | "json";
  required?: boolean;
  default_value?: string | number | boolean | null;
  options?: Array<{ value: string }>;
};

export const normalizeProviderConnectionConfig = (
  providerType: string,
  raw: Record<string, unknown>,
) => {
  if (providerType !== "wxpusher") {
    return raw;
  }

  return {
    ...raw,
    ...(!("app_token" in raw) && "appToken" in raw
      ? { app_token: raw.appToken }
      : {}),
    ...(!("server_url" in raw) && "serverUrl" in raw
      ? { server_url: raw.serverUrl }
      : {}),
    ...(!("timeout_seconds" in raw) && "timeoutSeconds" in raw
      ? { timeout_seconds: raw.timeoutSeconds }
      : {}),
  };
};

export const normalizeProviderTargetConfig = (
  providerType: string,
  raw: Record<string, unknown>,
) => {
  if (providerType !== "wxpusher") {
    return raw;
  }

  const topicValue =
    raw.topic_ids ??
    raw.topicIds ??
    raw.topic_id ??
    raw.topicId ??
    raw.topic ??
    raw.Topic;

  return {
    ...raw,
    ...(raw.topic_ids === undefined && topicValue !== undefined
      ? { topic_ids: topicValue }
      : {}),
    ...(!("verify_pay_type" in raw) && "verifyPayType" in raw
      ? { verify_pay_type: raw.verifyPayType }
      : {}),
  };
};

const normalizeJsonField = (value: unknown, fieldLabel: string) => {
  if (value === undefined || value === null || value === "") return undefined;
  if (typeof value === "string") {
    try {
      return JSON.parse(value) as unknown;
    } catch {
      throw new Error(serviceT("invalidJson", { field: fieldLabel }));
    }
  }
  return value;
};

export const normalizeSchemaPatch = (
  raw: Record<string, unknown>,
  fields: SchemaField[],
) => {
  const normalized: Record<string, unknown> = {};

  for (const field of fields) {
    if (!(field.key in raw)) continue;
    const input = raw[field.key];
    switch (field.type) {
      case "string":
        normalized[field.key] =
          input === undefined || input === null ? "" : String(input).trim();
        break;
      case "number":
        normalized[field.key] = parseNumberField(input, 0);
        break;
      case "boolean":
        normalized[field.key] = Boolean(input);
        break;
      case "select": {
        const value = String(input ?? "").trim();
        if (
          field.options?.length &&
          !field.options.some((option) => option.value === value)
        ) {
          throw new Error(
            serviceT("invalidSelectValue", { field: field.label }),
          );
        }
        normalized[field.key] = value;
        break;
      }
      case "json":
        normalized[field.key] = normalizeJsonField(input, field.label);
        break;
    }
  }

  return normalized;
};

export const applySchemaDefaults = (
  config: Record<string, unknown>,
  fields: Pick<SchemaField, "key" | "default_value">[],
) => {
  const next = { ...config };
  for (const field of fields) {
    if (next[field.key] !== undefined) continue;
    if (field.default_value === undefined) continue;
    next[field.key] = field.default_value;
  }
  return next;
};

export const validateRequiredSchemaFields = (
  config: Record<string, unknown>,
  fields: Pick<SchemaField, "key" | "label" | "required">[],
) => {
  for (const field of fields) {
    if (!field.required) continue;
    const value = config[field.key];
    if (value === undefined || value === null || value === "") {
      throw new Error(serviceT("fieldRequired", { field: field.label }));
    }
  }
};
