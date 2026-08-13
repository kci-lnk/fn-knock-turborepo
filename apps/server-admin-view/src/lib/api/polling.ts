import type { TrafficStats } from "../../types";
import { DashboardAPI } from "./dashboard";
import { DDNSAPI } from "./ddns";
import type { DDNSPollPayload } from "./ddns";
import {
  CloudflaredAPI,
  FrpcAPI,
  type CloudflaredPollPayload,
  type FrpcPollPayload,
} from "./tunnel";

export type PollTarget = "dashboard" | "ddns" | "frpc" | "cloudflared";

export type PollingPayloadMap = {
  dashboard: TrafficStats;
  ddns: DDNSPollPayload;
  frpc: FrpcPollPayload;
  cloudflared: CloudflaredPollPayload;
};

export const PollingAPI = {
  async poll<T extends PollTarget>(
    target: T,
    cursor?: number,
    signal?: AbortSignal,
  ): Promise<PollingPayloadMap[T]> {
    switch (target) {
      case "dashboard":
        return (await DashboardAPI.getRealtime(signal)) as PollingPayloadMap[T];
      case "ddns":
        return (await DDNSAPI.poll(cursor, signal)) as PollingPayloadMap[T];
      case "frpc":
        return (await FrpcAPI.poll(cursor, signal)) as PollingPayloadMap[T];
      case "cloudflared":
        return (await CloudflaredAPI.poll(
          cursor,
          signal,
        )) as PollingPayloadMap[T];
      default:
        throw new Error(`Unsupported poll target: ${String(target)}`);
    }
  },
};
