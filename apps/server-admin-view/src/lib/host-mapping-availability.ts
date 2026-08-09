import type { HostMapping, HostMappingAvailability } from "@/types";
import {
  formatDailyAvailabilityWindow,
  getAvailabilityWindowValidationError,
  isAvailabilityWindowOpen,
  isAvailabilityWindowValid,
  normalizeDailyAvailability,
  parseAvailabilityTimeToMinutes,
} from "./daily-availability";

export {
  getAvailabilityWindowValidationError,
  isAvailabilityWindowOpen,
  isAvailabilityWindowValid,
  parseAvailabilityTimeToMinutes,
};

export type HostMappingAvailabilityState =
  "enabled" | "disabled" | "scheduled_open" | "scheduled_closed";

export type HostMappingAvailabilityValidationError =
  "invalid_time" | "same_time";

export const normalizeHostMappingAvailability = (
  value?: Partial<HostMappingAvailability> | null,
): HostMappingAvailability | null => normalizeDailyAvailability(value);

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
  return formatDailyAvailabilityWindow(availability);
};
