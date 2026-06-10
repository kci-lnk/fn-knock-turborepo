import { DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES, ddnsManager } from ".";
import { runAutomaticDDNSCheck } from "./auto-check";

class DDNSIntervalScheduler {
  private timer: ReturnType<typeof setTimeout> | null = null;
  private running = false;
  private currentIntervalMinutes: number | null = null;

  async start(): Promise<void> {
    if (this.running) {
      return;
    }

    this.running = true;
    try {
      await this.reload();
    } catch (error) {
      this.running = false;
      throw error;
    }
  }

  stop(): void {
    this.running = false;
    this.clearTimer();
  }

  async reload(): Promise<void> {
    const settings = await ddnsManager.getSettings();
    this.schedule(settings.updateIntervalMinutes);
  }

  private clearTimer(): void {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }

  private schedule(intervalMinutes: number): void {
    this.clearTimer();
    this.currentIntervalMinutes = intervalMinutes;

    if (!this.running) {
      return;
    }

    this.timer = setTimeout(() => {
      void this.runAndReschedule();
    }, intervalMinutes * 60 * 1000);
  }

  private async runAndReschedule(): Promise<void> {
    try {
      await runAutomaticDDNSCheck({
        trigger: "cron",
        emitSkipLog: true,
      });
    } catch (error) {
      console.error("[ddns][scheduler] error:", error);
    } finally {
      if (!this.running) {
        return;
      }

      try {
        const settings = await ddnsManager.getSettings();
        this.schedule(settings.updateIntervalMinutes);
      } catch (error) {
        console.error("[ddns][scheduler] reload error:", error);
        this.schedule(
          this.currentIntervalMinutes || DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES,
        );
      }
    }
  }
}

export const ddnsIntervalScheduler = new DDNSIntervalScheduler();
