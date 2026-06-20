import { createHash, randomBytes } from "node:crypto";
import { tDefault } from "../../i18n";
import {
  DEFAULT_LOCALE,
  normalizeLocale,
  translate,
} from "../../../../../../packages/i18n/src";

export const nowIso = () => new Date().toISOString();

export const createId = (prefix: string) =>
  `${prefix}_${randomBytes(10).toString("hex")}`;

export const createStableId = (prefix: string, ...parts: string[]) =>
  `${prefix}_${createHash("sha256")
    .update(parts.join("\u0000"))
    .digest("hex")
    .slice(0, 24)}`;

const escapeRegExp = (value: string) =>
  value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

export const serviceT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.notifications.service.${key}`, params);

export const serviceTForLocale = (
  locale: string | null | undefined,
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) =>
  translate(
    normalizeLocale(locale) ?? DEFAULT_LOCALE,
    `server.notifications.service.${key}`,
    params,
  );

export const asPlainRecord = (value: unknown) => {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return {} as Record<string, unknown>;
  }
  return value as Record<string, unknown>;
};

export const uniqueStrings = (values: string[] | undefined) =>
  Array.from(
    new Set((values || []).map((value) => value.trim()).filter(Boolean)),
  );

export const buildNextSequentialName = (
  baseLabel: string,
  existingNames: string[],
) => {
  const normalizedBase = baseLabel.trim() || serviceT("unnamed");
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

export const parseNumberField = (
  value: unknown,
  fallback: number,
  options: { min?: number; max?: number } = {},
) => {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return fallback;
  const floored = Math.floor(parsed);
  if (options.min !== undefined && floored < options.min) {
    return options.min;
  }
  if (options.max !== undefined && floored > options.max) {
    return options.max;
  }
  return floored;
};

export const toMs = (value: string) => {
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) ? timestamp : 0;
};
