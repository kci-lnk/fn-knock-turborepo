import type { HostMapping, HostMappingAvailability } from "@/types";

export type HostMappingAvailabilityState =
  | "enabled"
  | "disabled"
  | "scheduled_open"
  | "scheduled_closed";

export type HostMappingAvailabilityValidationError =
  | "invalid_time"
  | "same_time";

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
): HostMappingAvailabilityValidationError | null => {
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

export const normalizeHostMappingAvailability = (
  value?: Partial<HostMappingAvailability> | null,
): HostMappingAvailability | null => {
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

export const isAvailabilityWindowOpen = (
  availability: HostMappingAvailability | null | undefined,
  now = new Date(),
): boolean => {
  if (availability?.enabled !== true) return true;
  const startMinute = parseAvailabilityTimeToMinutes(availability.start_time);
  const endMinute = parseAvailabilityTimeToMinutes(availability.end_time);
  if (startMinute === null || endMinute === null || startMinute === endMinute) {
    return true;
  }
  const currentMinute = now.getHours() * 60 + now.getMinutes();
  if (startMinute < endMinute) {
    return currentMinute >= startMinute && currentMinute < endMinute;
  }
  return currentMinute >= startMinute || currentMinute < endMinute;
};

export const getHostMappingAvailabilityState = (
  mapping: Pick<HostMapping, "disabled" | "availability">,
  now = new Date(),
): HostMappingAvailabilityState => {
  if (mapping.disabled === true) return "disabled";
  if (mapping.availability?.enabled === true) {
    return isAvailabilityWindowOpen(mapping.availability, now)
      ? "scheduled_open"
      : "scheduled_closed";
  }
  return "enabled";
};

export const isHostMappingUnavailable = (
  mapping: Pick<HostMapping, "disabled" | "availability">,
  now = new Date(),
): boolean => {
  const state = getHostMappingAvailabilityState(mapping, now);
  return state === "disabled" || state === "scheduled_closed";
};

export const formatHostMappingAvailabilityWindow = (
  mapping: Pick<HostMapping, "availability">,
): string => {
  const availability = mapping.availability;
  if (availability?.enabled !== true) return "";
  return `${availability.start_time.trim()}-${availability.end_time.trim()}`;
};
