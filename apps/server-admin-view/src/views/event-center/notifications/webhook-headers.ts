import type {
  NotificationHeaderConstraints,
  NotificationHeaderEntry,
} from "../../../types";

export const DEFAULT_WEBHOOK_HEADER_CONSTRAINTS: NotificationHeaderConstraints =
  {
    kind: "headers",
    max_items: 32,
    max_name_bytes: 128,
    max_value_bytes: 8 * 1024,
    max_total_bytes: 16 * 1024,
    reserved_names: [
      "host",
      "content-type",
      "content-length",
      "connection",
      "proxy-connection",
      "proxy-authenticate",
      "proxy-authorization",
      "http2-settings",
      "keep-alive",
      "transfer-encoding",
      "te",
      "trailer",
      "upgrade",
      "x-fn-knock-provider",
      "x-fn-knock-signature",
      "x-fn-knock-trace-id",
    ],
  };

export type WebhookHeaderValidationCode =
  | "tooMany"
  | "nameRequired"
  | "nameTooLong"
  | "invalidName"
  | "reservedName"
  | "duplicateName"
  | "valueTooLong"
  | "invalidValue"
  | "totalTooLarge";

export interface WebhookHeaderValidationIssue {
  code: WebhookHeaderValidationCode;
  row?: number;
  name?: string;
  max?: number;
}

const HEADER_NAME_PATTERN = /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/u;
const byteLength = (value: string) => new TextEncoder().encode(value).length;
const hasControlCharacter = (value: string) =>
  Array.from(value).some((character) => {
    const codePoint = character.codePointAt(0) || 0;
    return codePoint <= 31 || (codePoint >= 127 && codePoint <= 159);
  });

export const resolveWebhookHeaderConstraints = (
  constraints?: NotificationHeaderConstraints,
): NotificationHeaderConstraints => ({
  ...DEFAULT_WEBHOOK_HEADER_CONSTRAINTS,
  ...constraints,
  reserved_names:
    constraints?.reserved_names ||
    DEFAULT_WEBHOOK_HEADER_CONSTRAINTS.reserved_names,
});

export const coerceWebhookHeaderEntries = (
  value: unknown,
): NotificationHeaderEntry[] => {
  if (!Array.isArray(value)) return [];
  return value.map((entry) => {
    const object =
      entry && typeof entry === "object"
        ? (entry as Record<string, unknown>)
        : {};
    return {
      name: typeof object.name === "string" ? object.name : "",
      value: typeof object.value === "string" ? object.value : "",
    };
  });
};

export const normalizeWebhookHeaderEntries = (
  value: unknown,
): NotificationHeaderEntry[] =>
  coerceWebhookHeaderEntries(value).map((entry) => ({
    name: entry.name.trim(),
    value: entry.value.trim(),
  }));

export const validateWebhookHeaderEntries = (
  value: unknown,
  rawConstraints?: NotificationHeaderConstraints,
): WebhookHeaderValidationIssue[] => {
  const entries = coerceWebhookHeaderEntries(value);
  const constraints = resolveWebhookHeaderConstraints(rawConstraints);
  const issues: WebhookHeaderValidationIssue[] = [];
  const seenNames = new Set<string>();
  const reservedNames = new Set(
    constraints.reserved_names.map((name) => name.toLowerCase()),
  );
  let totalBytes = 0;

  if (entries.length > constraints.max_items) {
    issues.push({ code: "tooMany", max: constraints.max_items });
  }

  entries.forEach((entry, row) => {
    const name = entry.name.trim();
    const value = entry.value.trim();
    totalBytes += byteLength(name) + byteLength(value);

    if (!name) {
      issues.push({ code: "nameRequired", row });
      return;
    }
    if (byteLength(name) > constraints.max_name_bytes) {
      issues.push({
        code: "nameTooLong",
        row,
        name,
        max: constraints.max_name_bytes,
      });
    } else if (
      hasControlCharacter(entry.name) ||
      !HEADER_NAME_PATTERN.test(name)
    ) {
      issues.push({ code: "invalidName", row, name });
    }

    const normalizedName = name.toLowerCase();
    if (reservedNames.has(normalizedName)) {
      issues.push({ code: "reservedName", row, name });
    }
    if (seenNames.has(normalizedName)) {
      issues.push({ code: "duplicateName", row, name });
    } else {
      seenNames.add(normalizedName);
    }

    if (byteLength(value) > constraints.max_value_bytes) {
      issues.push({
        code: "valueTooLong",
        row,
        name,
        max: constraints.max_value_bytes,
      });
    } else if (hasControlCharacter(entry.value)) {
      issues.push({ code: "invalidValue", row, name });
    }
  });

  if (totalBytes > constraints.max_total_bytes) {
    issues.push({ code: "totalTooLarge", max: constraints.max_total_bytes });
  }
  return issues;
};
