import { DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES, ddnsManager } from ".";
import { runAutomaticDDNSCheck } from "./auto-check";
import { DDNSIntervalScheduler } from "./scheduler-core";

export const ddnsIntervalScheduler = new DDNSIntervalScheduler({
  getSettings: () => ddnsManager.getSettings(),
  runAutomaticDDNSCheck,
  fallbackIntervalMinutes: DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES,
});

export {
  DDNSIntervalScheduler,
  DEFAULT_DDNS_STARTUP_CHECK_DELAY_MS,
} from "./scheduler-core";
