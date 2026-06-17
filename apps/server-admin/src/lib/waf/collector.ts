import { goBackend } from "../go-backend";
import { configManager } from "../redis";
import { emitWAFBlockedEvent } from "../system-events/helpers";
import { tDefault } from "../i18n";
import { wafLogStore } from "./log-store";

const DEFAULT_DRAIN_LIMIT = 500;
const wafCollectorT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.wafCollector.${key}`, params);

type WAFDrainOnceResult = {
  drained: number;
  remaining: number;
};

export class WAFCollector {
  private timer: NodeJS.Timeout | null = null;
  private running = false;
  private draining: Promise<WAFDrainOnceResult> | null = null;

  start(): void {
    if (this.timer) return;
    this.schedule(0);
  }

  stop(): void {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }

  async drainOnce(limit = DEFAULT_DRAIN_LIMIT): Promise<WAFDrainOnceResult> {
    if (this.draining) return this.draining;
    this.draining = this.drainOnceInternal(limit).finally(() => {
      this.draining = null;
    });
    return this.draining;
  }

  private async drainOnceInternal(limit: number): Promise<WAFDrainOnceResult> {
    const config = await configManager.getWAFConfig();
    const response = await goBackend.drainWAFEvents(limit);
    if (!response.success || !response.data) {
      throw new Error(response.message || wafCollectorT("drainFailed"));
    }
    if (response.data.events.length > 0) {
      await wafLogStore.persistEvents(
        response.data.events,
        config.log_retention_days,
      );
      await Promise.all(
        response.data.events
          .filter((event) => isBlockingEvent(event))
          .map((event) =>
            emitWAFBlockedEvent({
              ip: event.client_ip || event.remote_addr || "unknown",
              traceId: event.trace_id,
              blockedAt: event.time,
              mode: String(event.mode || ""),
              action: event.action || "deny",
              status: event.status,
              host: event.host,
              path: event.path,
              requestUri: event.request_uri,
              routeType: event.route_type,
              routeKey: event.route_key,
              bundleId: event.bundle_id,
              ruleIds: event.rule_ids || [],
            }),
          ),
      );
    }
    return {
      drained: response.data.drained,
      remaining: response.data.remaining,
    };
  }

  private schedule(delayMs: number): void {
    this.timer = setTimeout(() => {
      void this.tick();
    }, delayMs);
  }

  private async tick(): Promise<void> {
    if (this.running) {
      this.scheduleNext().catch((error) => {
        console.error("[waf] failed to schedule collector:", error);
      });
      return;
    }
    this.running = true;
    try {
      await this.drainOnce();
    } catch (error) {
      console.error("[waf] failed to drain events:", error);
    } finally {
      this.running = false;
      await this.scheduleNext();
    }
  }

  private async scheduleNext(): Promise<void> {
    const config = await configManager.getWAFConfig();
    const delaySeconds = Math.max(
      1,
      Math.min(60, config.drain_interval_seconds),
    );
    this.schedule(delaySeconds * 1000);
  }
}

export const wafCollector = new WAFCollector();

const isBlockingEvent = (event: {
  action?: string;
  mode?: string;
  status?: number;
}) => {
  const action = String(event.action || "").toLowerCase();
  if (action === "block" || action === "deny") return true;
  if (action === "detect" || action === "log") return false;
  return (
    String(event.mode || "").toLowerCase() === "blocking" && !!event.status
  );
};
