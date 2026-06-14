import fs from "node:fs";
import { spawn } from "node:child_process";
import { collectStreamOutput, sleep } from "./runtime";
import {
  DEFAULT_LOCALE,
  type LocaleCode,
  type MessageParams,
  normalizeLocale,
  translate,
} from "../../../../packages/i18n/src";

const EXPECTED_TIME_ZONE = "Asia/Shanghai";
const TIME_DRIFT_THRESHOLD_MS = 90_000;
const CLOCK_CHECK_INTERVAL_MS = 10 * 60 * 1000;
const NETWORK_REQUEST_TIMEOUT_MS = 4_000;

const NETWORK_TIME_SOURCES = [
  { label: "Baidu HTTPS", url: "https://www.baidu.com/" },
  { label: "QQ HTTPS", url: "https://www.qq.com/" },
  { label: "Aliyun HTTPS", url: "https://www.aliyun.com/" },
  { label: "Baidu HTTP", url: "http://www.baidu.com/" },
  { label: "QQ HTTP", url: "http://www.qq.com/" },
  { label: "Aliyun HTTP", url: "http://www.aliyun.com/" },
] as const;

const resolveClockLocale = (locale: string | null | undefined): LocaleCode =>
  normalizeLocale(locale) ?? DEFAULT_LOCALE;

const clockT = (
  locale: string | null | undefined,
  key: string,
  params?: MessageParams,
) => translate(resolveClockLocale(locale), `server.systemClock.${key}`, params);

export type SystemClockIssueCode = "timezone_mismatch" | "time_mismatch";

export type SystemClockIssue = {
  code: SystemClockIssueCode;
  title: string;
  message: string;
};

export type SystemClockStatus = {
  expectedTimeZone: string;
  systemTimeZone: string | null;
  checkedAt: string | null;
  networkSource: string | null;
  hasRemoteTime: boolean;
  lastCheckError: string | null;
  systemTimeMs: number | null;
  remoteTimeMs: number | null;
  systemBeijingTime: string | null;
  remoteBeijingTime: string | null;
  driftMs: number | null;
  driftThresholdMs: number;
  timeMismatch: boolean;
  timezoneMismatch: boolean;
  needsAttention: boolean;
  issues: SystemClockIssue[];
  checking: boolean;
  syncInProgress: boolean;
  lastSyncAt: string | null;
  lastSyncError: string | null;
  syncSummary: string | null;
};

type NetworkTimeResult = {
  epochMs: number;
  source: string;
};

const toErrorMessage = (error: unknown, fallback: string) => {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  return fallback;
};

const formatBeijingTime = (epochMs: number | null, locale?: string | null) => {
  if (!Number.isFinite(epochMs)) return null;
  return new Intl.DateTimeFormat(resolveClockLocale(locale), {
    timeZone: EXPECTED_TIME_ZONE,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(new Date(epochMs as number));
};

const formatDrift = (driftMs: number, locale?: string | null) => {
  const totalSeconds = Math.max(1, Math.round(Math.abs(driftMs) / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;

  if (minutes <= 0) return clockT(locale, "duration.seconds", { seconds });
  if (seconds === 0) return clockT(locale, "duration.minutes", { minutes });
  return clockT(locale, "duration.minutesSeconds", { minutes, seconds });
};

const createInitialStatus = (): SystemClockStatus => ({
  expectedTimeZone: EXPECTED_TIME_ZONE,
  systemTimeZone: null,
  checkedAt: null,
  networkSource: null,
  hasRemoteTime: false,
  lastCheckError: null,
  systemTimeMs: null,
  remoteTimeMs: null,
  systemBeijingTime: null,
  remoteBeijingTime: null,
  driftMs: null,
  driftThresholdMs: TIME_DRIFT_THRESHOLD_MS,
  timeMismatch: false,
  timezoneMismatch: false,
  needsAttention: false,
  issues: [],
  checking: false,
  syncInProgress: false,
  lastSyncAt: null,
  lastSyncError: null,
  syncSummary: null,
});

export class SystemClockManager {
  private status: SystemClockStatus = createInitialStatus();
  private checkPromise: Promise<SystemClockStatus> | null = null;
  private syncPromise: Promise<{
    message: string;
    data: SystemClockStatus;
  }> | null = null;
  private pollTimer: NodeJS.Timeout | null = null;

  prepareOnBoot() {
    this.ensurePolling();
    void this.checkNow();
  }

  private buildIssues(
    status: Pick<
      SystemClockStatus,
      "timezoneMismatch" | "timeMismatch" | "systemTimeZone" | "driftMs"
    >,
    locale?: string | null,
  ): SystemClockIssue[] {
    const issues: SystemClockIssue[] = [];

    if (status.timezoneMismatch) {
      issues.push({
        code: "timezone_mismatch",
        title: clockT(locale, "issues.timezone.title"),
        message: clockT(locale, "issues.timezone.message", {
          timezone: status.systemTimeZone || clockT(locale, "unknown"),
          expected: EXPECTED_TIME_ZONE,
        }),
      });
    }

    if (status.timeMismatch && status.driftMs !== null) {
      issues.push({
        code: "time_mismatch",
        title: clockT(locale, "issues.timeMismatch.title"),
        message: clockT(locale, "issues.timeMismatch.message", {
          drift: formatDrift(status.driftMs, locale),
        }),
      });
    }

    return issues;
  }

  getStatus(locale?: string | null): SystemClockStatus {
    return {
      ...this.status,
      systemBeijingTime: formatBeijingTime(this.status.systemTimeMs, locale),
      remoteBeijingTime: formatBeijingTime(this.status.remoteTimeMs, locale),
      issues: this.buildIssues(this.status, locale),
    };
  }

  async checkNow(locale?: string | null): Promise<SystemClockStatus> {
    if (this.checkPromise) return this.checkPromise;
    const resolvedLocale = resolveClockLocale(locale);

    this.status = {
      ...this.status,
      checking: true,
      lastCheckError: null,
    };

    this.checkPromise = (async () => {
      const systemTimeZone = this.detectSystemTimeZone();
      const systemTimeMs = Date.now();
      let remote: NetworkTimeResult | null = null;
      let lastCheckError: string | null = null;

      try {
        remote = await this.fetchNetworkTime(resolvedLocale);
      } catch (error) {
        lastCheckError = toErrorMessage(
          error,
          clockT(resolvedLocale, "networkCheckFailed"),
        );
      }

      const remoteTimeMs = remote?.epochMs ?? null;
      const driftMs =
        remoteTimeMs === null ? null : systemTimeMs - remoteTimeMs;
      const timeMismatch =
        driftMs !== null && Math.abs(driftMs) > TIME_DRIFT_THRESHOLD_MS;
      const timezoneMismatch = systemTimeZone !== EXPECTED_TIME_ZONE;
      const issues = this.buildIssues(
        { timezoneMismatch, timeMismatch, systemTimeZone, driftMs },
        resolvedLocale,
      );

      this.status = {
        ...this.status,
        systemTimeZone,
        checkedAt: new Date().toISOString(),
        networkSource: remote?.source ?? null,
        hasRemoteTime: remoteTimeMs !== null,
        lastCheckError,
        systemTimeMs,
        remoteTimeMs,
        systemBeijingTime: formatBeijingTime(systemTimeMs, resolvedLocale),
        remoteBeijingTime: formatBeijingTime(remoteTimeMs, resolvedLocale),
        driftMs,
        driftThresholdMs: TIME_DRIFT_THRESHOLD_MS,
        timeMismatch,
        timezoneMismatch,
        needsAttention: timezoneMismatch || timeMismatch,
        issues,
        checking: false,
      };

      return this.getStatus(resolvedLocale);
    })().finally(() => {
      this.checkPromise = null;
    });

    return this.checkPromise;
  }

  async syncNow(
    locale?: string | null,
  ): Promise<{ message: string; data: SystemClockStatus }> {
    if (this.syncPromise) return this.syncPromise;
    const resolvedLocale = resolveClockLocale(locale);

    this.status = {
      ...this.status,
      syncInProgress: true,
      lastSyncError: null,
    };

    this.syncPromise = (async () => {
      const actions: string[] = [];

      try {
        const statusBeforeSync = await this.checkNow(resolvedLocale);

        if (statusBeforeSync.systemTimeZone !== EXPECTED_TIME_ZONE) {
          actions.push(await this.setSystemTimeZone(resolvedLocale));
        }

        if (
          statusBeforeSync.hasRemoteTime &&
          statusBeforeSync.remoteTimeMs !== null
        ) {
          const checkedAtMs = statusBeforeSync.checkedAt
            ? Date.parse(statusBeforeSync.checkedAt)
            : Date.now();
          const elapsedMs = Math.max(0, Date.now() - checkedAtMs);
          const targetEpochMs = statusBeforeSync.remoteTimeMs + elapsedMs;
          actions.push(
            await this.setSystemClock(targetEpochMs, resolvedLocale),
          );
        }

        const ntpMessage = await this.enableNetworkTimeSync(resolvedLocale);
        if (ntpMessage) {
          actions.push(ntpMessage);
        }

        await sleep(1_500);
        const nextStatus = await this.checkNow(resolvedLocale);
        const message =
          actions.length > 0
            ? actions.join(clockT(resolvedLocale, "actionSeparator"))
            : clockT(resolvedLocale, "statusRefreshed");

        this.status = {
          ...nextStatus,
          syncInProgress: false,
          lastSyncAt: new Date().toISOString(),
          lastSyncError: null,
          syncSummary: message,
        };

        return {
          message,
          data: this.getStatus(resolvedLocale),
        };
      } catch (error) {
        const message = toErrorMessage(
          error,
          clockT(resolvedLocale, "syncFailed"),
        );
        this.status = {
          ...this.status,
          syncInProgress: false,
          lastSyncAt: new Date().toISOString(),
          lastSyncError: message,
          syncSummary: null,
        };
        throw new Error(message);
      }
    })().finally(() => {
      this.syncPromise = null;
    });

    return this.syncPromise;
  }

  private ensurePolling() {
    if (this.pollTimer) return;

    this.pollTimer = setInterval(() => {
      void this.checkNow();
    }, CLOCK_CHECK_INTERVAL_MS);

    this.pollTimer.unref?.();
  }

  private detectSystemTimeZone() {
    try {
      const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone?.trim();
      return timeZone || null;
    } catch {
      return null;
    }
  }

  private async fetchNetworkTime(
    locale?: string | null,
  ): Promise<NetworkTimeResult> {
    let lastError = clockT(locale, "networkTimeUnavailable");

    for (const source of NETWORK_TIME_SOURCES) {
      try {
        return await this.fetchNetworkTimeFromSource(
          source.url,
          source.label,
          locale,
        );
      } catch (error) {
        lastError = toErrorMessage(
          error,
          clockT(locale, "sourceFetchFailed", { source: source.label }),
        );
      }
    }

    throw new Error(lastError);
  }

  private async fetchNetworkTimeFromSource(
    url: string,
    label: string,
    locale?: string | null,
  ): Promise<NetworkTimeResult> {
    const requestStartedAt = Date.now();
    let response = await fetch(url, {
      method: "HEAD",
      redirect: "manual",
      signal: AbortSignal.timeout(NETWORK_REQUEST_TIMEOUT_MS),
      headers: {
        "Cache-Control": "no-cache",
        Pragma: "no-cache",
      },
    }).catch(() => null);

    let dateHeader = response?.headers.get("date") ?? null;

    if (!dateHeader) {
      response = await fetch(url, {
        method: "GET",
        redirect: "manual",
        signal: AbortSignal.timeout(NETWORK_REQUEST_TIMEOUT_MS),
        headers: {
          "Cache-Control": "no-cache",
          Pragma: "no-cache",
        },
      });
      dateHeader = response.headers.get("date");
    }

    if (!dateHeader) {
      throw new Error(clockT(locale, "missingDateHeader", { source: label }));
    }

    const remoteTimeMs = Date.parse(dateHeader);
    if (!Number.isFinite(remoteTimeMs)) {
      throw new Error(clockT(locale, "invalidDateHeader", { source: label }));
    }

    const latencyMs = Math.max(0, Date.now() - requestStartedAt);

    return {
      epochMs: remoteTimeMs + Math.round(latencyMs / 2),
      source: label,
    };
  }

  private async runCommand(
    command: string,
    args: string[],
    locale?: string | null,
  ) {
    const proc = spawn(command, args, {
      stdio: ["ignore", "pipe", "pipe"],
    });

    const exitCodePromise = new Promise<number>((resolve, reject) => {
      proc.once("error", reject);
      proc.once("close", (code) => resolve(code ?? -1));
    });

    const [stdout, stderr, exitCode] = await Promise.all([
      collectStreamOutput(proc.stdout),
      collectStreamOutput(proc.stderr),
      exitCodePromise,
    ]);

    if (exitCode !== 0) {
      throw new Error(
        stderr.trim() ||
          stdout.trim() ||
          clockT(locale, "commandFailed", { command }),
      );
    }

    return {
      stdout: stdout.trim(),
      stderr: stderr.trim(),
    };
  }

  private async tryRunCommand(
    command: string,
    args: string[],
    locale?: string | null,
  ) {
    try {
      await this.runCommand(command, args, locale);
      return true;
    } catch {
      return false;
    }
  }

  private async setSystemTimeZone(locale?: string | null) {
    try {
      await this.runCommand(
        "timedatectl",
        ["set-timezone", EXPECTED_TIME_ZONE],
        locale,
      );
      return clockT(locale, "timezoneSet", { timezone: EXPECTED_TIME_ZONE });
    } catch {
      const zoneinfoPath = `/usr/share/zoneinfo/${EXPECTED_TIME_ZONE}`;
      if (!fs.existsSync(zoneinfoPath)) {
        throw new Error(
          clockT(locale, "missingZoneinfoFile", { path: zoneinfoPath }),
        );
      }

      try {
        fs.rmSync("/etc/localtime", { force: true });
      } catch {
        // ignore and continue with overwrite attempt below
      }

      try {
        fs.symlinkSync(zoneinfoPath, "/etc/localtime");
      } catch {
        fs.copyFileSync(zoneinfoPath, "/etc/localtime");
      }

      fs.writeFileSync("/etc/timezone", `${EXPECTED_TIME_ZONE}\n`, "utf-8");
      return clockT(locale, "timezoneWritten", {
        timezone: EXPECTED_TIME_ZONE,
      });
    }
  }

  private async setSystemClock(targetEpochMs: number, locale?: string | null) {
    const targetSeconds = Math.floor(targetEpochMs / 1000);
    await this.runCommand("date", ["-u", "-s", `@${targetSeconds}`], locale);
    await this.tryRunCommand("hwclock", ["--systohc"], locale);
    return clockT(locale, "clockAdjusted");
  }

  private async enableNetworkTimeSync(locale?: string | null) {
    const actions: string[] = [];

    if (await this.tryRunCommand("timedatectl", ["set-ntp", "true"], locale)) {
      actions.push(clockT(locale, "ntpEnabled"));
    }

    for (const service of ["systemd-timesyncd", "chrony", "chronyd", "ntp"]) {
      if (await this.tryRunCommand("systemctl", ["restart", service], locale)) {
        actions.push(clockT(locale, "serviceRestarted", { service }));
        break;
      }
    }

    return actions.length > 0
      ? actions.join(clockT(locale, "listSeparator"))
      : null;
  }
}

export const systemClockManager = new SystemClockManager();
