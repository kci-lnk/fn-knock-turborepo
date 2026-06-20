export const DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES = 10;
export const MIN_DDNS_UPDATE_INTERVAL_MINUTES = 5;
export const MAX_DDNS_UPDATE_INTERVAL_MINUTES = 1440;

export const normalizeUpdateIntervalMinutes = (
  value: unknown,
  fallback = DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES,
): number => {
  const parsed =
    typeof value === "number"
      ? value
      : typeof value === "string"
        ? Number(value.trim())
        : NaN;

  if (
    Number.isInteger(parsed) &&
    parsed >= MIN_DDNS_UPDATE_INTERVAL_MINUTES &&
    parsed <= MAX_DDNS_UPDATE_INTERVAL_MINUTES
  ) {
    return parsed;
  }

  return fallback;
};

export const parseUpdateIntervalMinutesInput = (
  value: unknown,
): number | null => {
  if (typeof value === "string" && !/^\d+$/.test(value.trim())) {
    return null;
  }

  const parsed = typeof value === "number" ? value : Number(value);
  if (
    Number.isInteger(parsed) &&
    parsed >= MIN_DDNS_UPDATE_INTERVAL_MINUTES &&
    parsed <= MAX_DDNS_UPDATE_INTERVAL_MINUTES
  ) {
    return parsed;
  }

  return null;
};

const parseLegacyDDNSCronIntervalMinutes = (
  pattern: string | null | undefined,
): number | null => {
  const parts = pattern?.trim().split(/\s+/).filter(Boolean) || [];
  if (parts.length !== 5 && parts.length !== 6) {
    return null;
  }

  const minutePart = parts.length === 6 ? parts[1] : parts[0];
  const otherParts = parts.length === 6 ? parts.slice(2) : parts.slice(1);
  if (!minutePart) {
    return null;
  }
  if (parts.length === 6 && parts[0] !== "0") {
    return null;
  }

  if (!otherParts.every((part) => part === "*")) {
    return null;
  }

  const match = minutePart.match(/^\*\/(\d+)$/);
  if (!match) {
    return null;
  }

  const interval = match[1];
  if (!interval) {
    return null;
  }

  const minutes = Number.parseInt(interval, 10);
  if (
    !Number.isInteger(minutes) ||
    minutes < MIN_DDNS_UPDATE_INTERVAL_MINUTES ||
    minutes > MAX_DDNS_UPDATE_INTERVAL_MINUTES
  ) {
    return null;
  }

  return minutes;
};

export const getDefaultUpdateIntervalMinutes = () =>
  parseLegacyDDNSCronIntervalMinutes(process.env.DDNS_CRON) ??
  DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES;
