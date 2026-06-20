import { oidcT } from "./messages";

const LOGIN_ERROR_MESSAGE_MAX_LENGTH = 240;

export const normalizeString = (value: unknown) =>
  typeof value === "string" ? value.trim() : "";

export const normalizeOptionalString = (value: unknown) => {
  const normalized = normalizeString(value);
  return normalized || undefined;
};

export const normalizeLoginErrorMessage = (value: unknown) => {
  const normalized = normalizeString(value) || oidcT("loginFailedRetry");
  return normalized.length > LOGIN_ERROR_MESSAGE_MAX_LENGTH
    ? `${normalized.slice(0, LOGIN_ERROR_MESSAGE_MAX_LENGTH)}...`
    : normalized;
};

export const normalizeStringRecord = (value: unknown) => {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }
  const record: Record<string, string> = {};
  for (const [key, entry] of Object.entries(value)) {
    const normalizedKey = key.trim();
    const normalizedValue = normalizeString(entry);
    if (normalizedKey && normalizedValue) {
      record[normalizedKey] = normalizedValue;
    }
  }
  return Object.keys(record).length ? record : undefined;
};
