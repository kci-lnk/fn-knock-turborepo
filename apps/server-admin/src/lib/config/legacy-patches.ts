import type Redis from "ioredis";
import {
  normalizeEventSystemConfig,
  normalizeReverseProxyThrottleConfig,
} from "./app-config";
import { DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG } from "./defaults";
import type {
  AppConfig,
  EventSystemResourceAlertRuleConfig,
  ReverseProxyThrottleConfig,
} from "./types";

export const LEGACY_REVERSE_PROXY_THROTTLE_PATCH_FLAG_KEY =
  "fn_knock:patch:reverse-proxy-throttle:v1";
export const LEGACY_EVENT_SYSTEM_RESOURCE_ALERTS_PATCH_FLAG_KEY =
  "fn_knock:patch:event-system-resource-alerts:v1";

const LEGACY_DISABLED_CPU_ALERT_RULE: EventSystemResourceAlertRuleConfig = {
  enabled: false,
  threshold_percent: 85,
  recover_percent: 70,
  sample_interval_seconds: 15,
  sustain_seconds: 120,
};
const LEGACY_DISABLED_MEMORY_ALERT_RULE: EventSystemResourceAlertRuleConfig = {
  enabled: false,
  threshold_percent: 90,
  recover_percent: 75,
  sample_interval_seconds: 15,
  sustain_seconds: 120,
};

const LEGACY_REVERSE_PROXY_THROTTLE_CONFIG: Pick<
  ReverseProxyThrottleConfig,
  "requests_per_second" | "burst" | "block_seconds"
> = {
  requests_per_second: 20,
  burst: 50,
  block_seconds: 30,
};

const isSameResourceAlertRule = (
  currentRule: EventSystemResourceAlertRuleConfig,
  targetRule: EventSystemResourceAlertRuleConfig,
) =>
  currentRule.enabled === targetRule.enabled &&
  currentRule.threshold_percent === targetRule.threshold_percent &&
  currentRule.recover_percent === targetRule.recover_percent &&
  currentRule.sample_interval_seconds === targetRule.sample_interval_seconds &&
  currentRule.sustain_seconds === targetRule.sustain_seconds;

interface LegacyPatchContext {
  redis: Redis;
  configKey: string;
  patchFlagKey: string;
  getConfig: () => Promise<AppConfig>;
}

export const applyLegacyReverseProxyThrottlePatchIfNeeded = async ({
  redis,
  configKey,
  patchFlagKey,
  getConfig,
}: LegacyPatchContext): Promise<{
  applied: boolean;
  config: AppConfig;
}> => {
  const [config, patchFlag] = await Promise.all([
    getConfig(),
    redis.get(patchFlagKey),
  ]);

  if (patchFlag === "1") {
    return { applied: false, config };
  }

  const currentThrottle = normalizeReverseProxyThrottleConfig(
    config.reverse_proxy_throttle,
  );
  const shouldPatch =
    currentThrottle.requests_per_second ===
      LEGACY_REVERSE_PROXY_THROTTLE_CONFIG.requests_per_second &&
    currentThrottle.burst === LEGACY_REVERSE_PROXY_THROTTLE_CONFIG.burst &&
    currentThrottle.block_seconds ===
      LEGACY_REVERSE_PROXY_THROTTLE_CONFIG.block_seconds;

  if (!shouldPatch) {
    await redis.set(patchFlagKey, "1");
    return { applied: false, config };
  }

  const nextConfig: AppConfig = {
    ...config,
    reverse_proxy_throttle: {
      ...currentThrottle,
      requests_per_second:
        DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG.requests_per_second,
      burst: DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG.burst,
      block_seconds: DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG.block_seconds,
    },
  };

  await redis
    .multi()
    .set(configKey, JSON.stringify(nextConfig))
    .set(patchFlagKey, "1")
    .exec();

  return {
    applied: true,
    config: nextConfig,
  };
};

export const applyLegacyEventSystemResourceAlertsPatchIfNeeded = async ({
  redis,
  configKey,
  patchFlagKey,
  getConfig,
}: LegacyPatchContext): Promise<{
  applied: boolean;
  config: AppConfig;
}> => {
  const [config, patchFlag] = await Promise.all([
    getConfig(),
    redis.get(patchFlagKey),
  ]);

  if (patchFlag === "1") {
    return { applied: false, config };
  }

  const currentEventSystem = normalizeEventSystemConfig(config.event_system);
  const currentCpuRule = currentEventSystem.rules.cpu_alert;
  const currentMemoryRule = currentEventSystem.rules.memory_alert;
  const shouldPatch =
    isSameResourceAlertRule(currentCpuRule, LEGACY_DISABLED_CPU_ALERT_RULE) &&
    isSameResourceAlertRule(
      currentMemoryRule,
      LEGACY_DISABLED_MEMORY_ALERT_RULE,
    );

  if (!shouldPatch) {
    await redis.set(patchFlagKey, "1");
    return { applied: false, config };
  }

  const nextConfig: AppConfig = {
    ...config,
    event_system: {
      ...currentEventSystem,
      rules: {
        ...currentEventSystem.rules,
        cpu_alert: {
          ...currentCpuRule,
          enabled: true,
        },
        memory_alert: {
          ...currentMemoryRule,
          enabled: true,
        },
      },
    },
  };

  await redis
    .multi()
    .set(configKey, JSON.stringify(nextConfig))
    .set(patchFlagKey, "1")
    .exec();

  return {
    applied: true,
    config: nextConfig,
  };
};
