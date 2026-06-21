import type { RunAutomaticDDNSCheckOptions } from "./auto-check";
import { DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES } from "./update-interval";

export const DEFAULT_DDNS_STARTUP_CHECK_DELAY_MS = 30_000;

type SchedulerTimer = ReturnType<typeof setTimeout>;

type DDNSIntervalSchedulerDependencies = {
  getSettings: () => Promise<{ updateIntervalMinutes: number }>;
  runAutomaticDDNSCheck: (
    options?: RunAutomaticDDNSCheckOptions,
  ) => Promise<void> | void;
  fallbackIntervalMinutes?: number;
  startupCheckDelayMs?: number;
  logger?: Pick<Console, "error">;
  setTimeoutFn?: (callback: () => void, delayMs: number) => SchedulerTimer;
  clearTimeoutFn?: (timer: SchedulerTimer) => void;
};

export class DDNSIntervalScheduler {
  private timer: SchedulerTimer | null = null;
  private startupTimer: SchedulerTimer | null = null;
  private running = false;
  private currentIntervalMinutes: number | null = null;
  private readonly fallbackIntervalMinutes: number;
  private readonly startupCheckDelayMs: number;
  private readonly logger: Pick<Console, "error">;
  private readonly setTimeoutFn: (
    callback: () => void,
    delayMs: number,
  ) => SchedulerTimer;
  private readonly clearTimeoutFn: (timer: SchedulerTimer) => void;

  constructor(private readonly dependencies: DDNSIntervalSchedulerDependencies) {
    this.fallbackIntervalMinutes =
      dependencies.fallbackIntervalMinutes ??
      DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES;
    this.startupCheckDelayMs =
      dependencies.startupCheckDelayMs ?? DEFAULT_DDNS_STARTUP_CHECK_DELAY_MS;
    this.logger = dependencies.logger ?? console;
    this.setTimeoutFn = dependencies.setTimeoutFn ?? setTimeout;
    this.clearTimeoutFn = dependencies.clearTimeoutFn ?? clearTimeout;
  }

  async start(): Promise<void> {
    if (this.running) {
      return;
    }

    this.running = true;
    try {
      await this.reload();
      this.scheduleStartupCheck();
    } catch (error) {
      this.running = false;
      this.clearTimer();
      this.clearStartupTimer();
      throw error;
    }
  }

  stop(): void {
    this.running = false;
    this.clearTimer();
    this.clearStartupTimer();
  }

  async reload(): Promise<void> {
    const settings = await this.dependencies.getSettings();
    this.schedule(settings.updateIntervalMinutes);
  }

  private clearTimer(): void {
    if (this.timer) {
      this.clearTimeoutFn(this.timer);
      this.timer = null;
    }
  }

  private clearStartupTimer(): void {
    if (this.startupTimer) {
      this.clearTimeoutFn(this.startupTimer);
      this.startupTimer = null;
    }
  }

  private schedule(intervalMinutes: number): void {
    this.clearTimer();
    this.currentIntervalMinutes = intervalMinutes;

    if (!this.running) {
      return;
    }

    this.timer = this.setTimeoutFn(() => {
      void this.runAndReschedule();
    }, intervalMinutes * 60 * 1000);
  }

  private scheduleStartupCheck(): void {
    this.clearStartupTimer();

    if (!this.running) {
      return;
    }

    this.startupTimer = this.setTimeoutFn(() => {
      void this.runStartupCheck();
    }, this.startupCheckDelayMs);
  }

  private async runStartupCheck(): Promise<void> {
    this.startupTimer = null;

    if (!this.running) {
      return;
    }

    try {
      await this.dependencies.runAutomaticDDNSCheck({
        trigger: "startup",
        emitSkipLog: false,
        emitNoopLog: false,
      });
    } catch (error) {
      this.logger.error("[ddns][scheduler] startup check error:", error);
    }
  }

  private async runAndReschedule(): Promise<void> {
    try {
      await this.dependencies.runAutomaticDDNSCheck({
        trigger: "cron",
        emitSkipLog: true,
      });
    } catch (error) {
      this.logger.error("[ddns][scheduler] error:", error);
    } finally {
      if (!this.running) {
        return;
      }

      try {
        const settings = await this.dependencies.getSettings();
        this.schedule(settings.updateIntervalMinutes);
      } catch (error) {
        this.logger.error("[ddns][scheduler] reload error:", error);
        this.schedule(
          this.currentIntervalMinutes || this.fallbackIntervalMinutes,
        );
      }
    }
  }
}
