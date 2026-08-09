import type { DailyAvailability } from "@/types";

export type DailyAvailabilityValidationError = "invalid_time" | "same_time";

export const parseAvailabilityTimeToMinutes = (
  value: string,
): number | null => {
  const trimmed = value.trim();
  if (!/^\d{2}:\d{2}$/.test(trimmed)) return null;
  const [hourText, minuteText] = trimmed.split(":");
  const hour = Number.parseInt(hourText || "", 10);
  const minute = Number.parseInt(minuteText || "", 10);
  if (
    !Number.isFinite(hour) ||
    !Number.isFinite(minute) ||
    hour < 0 ||
    hour > 23 ||
    minute < 0 ||
    minute > 59
  ) {
    return null;
  }
  return hour * 60 + minute;
};

export const getAvailabilityWindowValidationError = (
  startTime: string,
  endTime: string,
): DailyAvailabilityValidationError | null => {
  const startMinute = parseAvailabilityTimeToMinutes(startTime);
  const endMinute = parseAvailabilityTimeToMinutes(endTime);
  if (startMinute === null || endMinute === null) return "invalid_time";
  if (startMinute === endMinute) return "same_time";
  return null;
};

export const isAvailabilityWindowValid = (
  startTime: string,
  endTime: string,
): boolean => {
  return getAvailabilityWindowValidationError(startTime, endTime) === null;
};

export const normalizeDailyAvailability = (
  value?: Partial<DailyAvailability> | null,
): DailyAvailability | null => {
  if (value?.enabled !== true) return null;
  const startTime =
    typeof value.start_time === "string" ? value.start_time.trim() : "";
  const endTime =
    typeof value.end_time === "string" ? value.end_time.trim() : "";
  if (!isAvailabilityWindowValid(startTime, endTime)) return null;
  return {
    enabled: true,
    start_time: startTime,
    end_time: endTime,
  };
};

const currentMinuteForTimeZone = (
  now: Date,
  timeZone?: string | null,
): number => {
  if (!timeZone) return now.getHours() * 60 + now.getMinutes();
  try {
    const parts = new Intl.DateTimeFormat("en-US", {
      hour: "2-digit",
      hourCycle: "h23",
      minute: "2-digit",
      timeZone,
    }).formatToParts(now);
    const hour = Number(parts.find((part) => part.type === "hour")?.value);
    const minute = Number(parts.find((part) => part.type === "minute")?.value);
    if (Number.isFinite(hour) && Number.isFinite(minute)) {
      return hour * 60 + minute;
    }
  } catch {
    // Fall back to the browser timezone if the server reports an unsupported zone.
  }
  return now.getHours() * 60 + now.getMinutes();
};

export const isAvailabilityWindowOpen = (
  availability: DailyAvailability | null | undefined,
  now = new Date(),
  timeZone?: string | null,
): boolean => {
  if (availability?.enabled !== true) return true;
  const startMinute = parseAvailabilityTimeToMinutes(availability.start_time);
  const endMinute = parseAvailabilityTimeToMinutes(availability.end_time);
  if (startMinute === null || endMinute === null || startMinute === endMinute) {
    return true;
  }
  const currentMinute = currentMinuteForTimeZone(now, timeZone);
  if (startMinute < endMinute) {
    return currentMinute >= startMinute && currentMinute < endMinute;
  }
  return currentMinute >= startMinute || currentMinute < endMinute;
};

export const formatDailyAvailabilityWindow = (
  availability: DailyAvailability | null | undefined,
): string => {
  if (availability?.enabled !== true) return "";
  return `${availability.start_time.trim()}-${availability.end_time.trim()}`;
};
