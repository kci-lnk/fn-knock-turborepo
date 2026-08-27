import type { ReverseProxyThrottleConfig } from "@/types";

export const DEFAULT_REVERSE_PROXY_THROTTLE = {
  enabled: true,
  requests_per_second: 500,
  burst: 1000,
  block_seconds: 30,
} satisfies ReverseProxyThrottleConfig;
