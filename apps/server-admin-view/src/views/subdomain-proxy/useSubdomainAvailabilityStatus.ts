import { ref } from "vue";
import {
  formatHostMappingAvailabilityWindow,
  getHostMappingAvailabilityState,
  isHostMappingUnavailable,
} from "@/lib/host-mapping-availability";
import type { HostMapping } from "@/types";

export const useSubdomainAvailabilityStatus = ({
  intervalMs = 60_000,
}: {
  intervalMs?: number;
} = {}) => {
  const availabilityClock = ref(Date.now());
  let availabilityClockTimer: number | null = null;

  const refreshAvailabilityClock = () => {
    availabilityClock.value = Date.now();
  };

  const getAvailabilityNow = () => new Date(availabilityClock.value);

  const getAvailabilityState = (mapping: HostMapping) =>
    getHostMappingAvailabilityState(mapping, getAvailabilityNow());

  const isMappingUnavailable = (mapping: HostMapping) =>
    isHostMappingUnavailable(mapping, getAvailabilityNow());

  const formatAvailabilityWindow = (mapping: HostMapping) =>
    formatHostMappingAvailabilityWindow(mapping);

  const stopAvailabilityClock = () => {
    if (availabilityClockTimer !== null) {
      window.clearInterval(availabilityClockTimer);
      availabilityClockTimer = null;
    }
  };

  const startAvailabilityClock = () => {
    stopAvailabilityClock();
    refreshAvailabilityClock();
    availabilityClockTimer = window.setInterval(
      refreshAvailabilityClock,
      intervalMs,
    );
  };

  return {
    formatAvailabilityWindow,
    getAvailabilityState,
    isMappingUnavailable,
    startAvailabilityClock,
    stopAvailabilityClock,
  };
};
