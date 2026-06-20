import type {
  NotificationDelivery,
  NotificationDeliveryPolicy,
} from "../types";
import { parseNumberField } from "./common";

export const DEFAULT_DELIVERY_POLICY: Required<NotificationDeliveryPolicy> = {
  timeout_seconds: 5,
  max_attempts: 3,
  backoff_seconds: 30,
};

export const resolveDeliveryPolicy = (
  policy?: NotificationDeliveryPolicy | null,
): Required<NotificationDeliveryPolicy> => ({
  timeout_seconds: parseNumberField(
    policy?.timeout_seconds,
    DEFAULT_DELIVERY_POLICY.timeout_seconds,
    { min: 1, max: 30 },
  ),
  max_attempts: parseNumberField(
    policy?.max_attempts,
    DEFAULT_DELIVERY_POLICY.max_attempts,
    { min: 1, max: 10 },
  ),
  backoff_seconds: parseNumberField(
    policy?.backoff_seconds,
    DEFAULT_DELIVERY_POLICY.backoff_seconds,
    { min: 5, max: 3600 },
  ),
});

export const isTerminalDeliveryStatus = (
  status: NotificationDelivery["status"],
) => status === "success" || status === "gave_up" || status === "skipped";

export const resolveDeliveryReadyAtMs = (delivery: NotificationDelivery) => {
  const nextRetryAtMs = delivery.next_retry_at
    ? Date.parse(delivery.next_retry_at)
    : NaN;
  if (Number.isFinite(nextRetryAtMs)) {
    return nextRetryAtMs;
  }

  const triggeredAtMs = Date.parse(delivery.triggered_at);
  return Number.isFinite(triggeredAtMs) ? triggeredAtMs : Date.now();
};
