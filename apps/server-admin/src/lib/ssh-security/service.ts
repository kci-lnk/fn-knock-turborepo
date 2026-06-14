import { isIP } from "node:net";
import { configManager } from "../redis";
import { ipLocationRefs, ipLocationService } from "../ip-location";
import { isWhitelistExemptIp, normalizeIp } from "../ip-normalize";
import {
  emitSSHIPBlockedEvent,
  emitSSHLoginFailureEvent,
  emitSSHLoginSuccessEvent,
} from "../system-events/helpers";
import {
  buildCIDRMatcher,
  sshSecurityDurationToSeconds,
  type SSHSecurityConfigInput,
} from "./config";
import { compileSSHSecurityConfig } from "./compiler";
import { sshSecurityFirewall } from "./firewall";
import { sshLogSource, type SSHLogFollowHandle } from "./log-source";
import { sshSecurityStore } from "./store";
import type {
  SSHLoginLogEntry,
  SSHLoginLogListResult,
  SSHSecurityBlockListResult,
  SSHSecurityBlockRecord,
  SSHSecurityBlockReason,
  SSHSecurityConfig,
  SSHSecurityDetails,
  SSHSecurityFirewallClearResult,
  SSHSecurityFirewallSyncResult,
  SSHSecurityRuntimeState,
} from "./types";
import {
  getCapabilityUnavailableMessage,
  getRuntimeCapabilities,
  getRuntimeProfile,
} from "../runtime-profile";
import { tDefault } from "../i18n";

type CIDRMatcher = ReturnType<typeof buildCIDRMatcher>;
const sshSecurityT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.sshSecurity.${key}`, params);

const normalizePage = (value: unknown): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 1;
};

const normalizeLimit = (value: unknown, fallback = 20): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed) || parsed <= 0) return fallback;
  return Math.min(100, parsed);
};

const blockReasonTags: Record<SSHSecurityBlockReason, string> = {
  failed_login_threshold: "ssh-failed-login-threshold",
  cidr_not_allowed: "ssh-cidr-not-allowed",
};

const STARTUP_BACKFILL_LOG_LIMIT = 2000;
const SUCCESS_LOG_COALESCE_WINDOW_MS = 30 * 1000;

const mergePorts = (...portLists: Array<Iterable<unknown> | undefined>) =>
  [
    ...new Set(
      portLists
        .flatMap((ports) => [...(ports ?? [])])
        .map((port) => Number.parseInt(String(port ?? ""), 10))
        .filter((port) => Number.isInteger(port) && port > 0 && port <= 65535),
    ),
  ].sort((left, right) => left - right);

const entryTimeMs = (entry: SSHLoginLogEntry): number => {
  const parsed = Date.parse(entry.happened_at);
  return Number.isFinite(parsed) ? parsed : 0;
};

const successCoalesceKey = (entry: SSHLoginLogEntry): string =>
  [
    entry.source,
    entry.outcome,
    entry.username,
    entry.ip,
    entry.auth_method ?? "",
  ].join("|");

const entryPorts = (entry: SSHLoginLogEntry): number[] =>
  mergePorts(entry.related_ports, entry.port ? [entry.port] : []);

const coalesceSuccessLoginLogs = (
  entries: SSHLoginLogEntry[],
): SSHLoginLogEntry[] => {
  const result: SSHLoginLogEntry[] = [];
  const latestByKey = new Map<string, SSHLoginLogEntry>();

  for (const entry of entries) {
    if (entry.outcome !== "success") {
      result.push(entry);
      continue;
    }

    const key = successCoalesceKey(entry);
    const existing = latestByKey.get(key);
    if (
      !existing ||
      Math.abs(entryTimeMs(existing) - entryTimeMs(entry)) >
        SUCCESS_LOG_COALESCE_WINDOW_MS
    ) {
      const next: SSHLoginLogEntry = {
        ...entry,
        repeat_count: Math.max(1, entry.repeat_count ?? 1),
        related_ports: entryPorts(entry),
      };
      result.push(next);
      latestByKey.set(key, next);
      continue;
    }

    existing.repeat_count =
      (existing.repeat_count ?? 1) + Math.max(1, entry.repeat_count ?? 1);
    existing.related_ports = mergePorts(
      existing.related_ports,
      entryPorts(entry),
    );
    if (entry.raw && !existing.raw.includes(entry.raw)) {
      existing.raw = `${existing.raw}\n${entry.raw}`;
    }
  }

  return result;
};

export class SSHSecurityService {
  private followHandle: SSHLogFollowHandle | null = null;
  private expiryTimer: ReturnType<typeof setTimeout> | null = null;
  private runtime: SSHSecurityRuntimeState | null = null;
  private cidrMatcher: CIDRMatcher | null = null;
  private starting = false;

  async getDetails(): Promise<SSHSecurityDetails> {
    const [config, runtime, activeBlockCount, availability, sshPorts] =
      await Promise.all([
        configManager.getSSHSecurityConfig(),
        sshSecurityStore.getRuntimeState(),
        sshSecurityStore.activeBlockCount(),
        this.getAvailability(),
        sshSecurityFirewall.getSSHPorts().catch(() => [22]),
      ]);

    return {
      config,
      summary: {
        configured: Boolean(config.configured_at),
        enabled: config.enabled,
        allowed_cidr_count: runtime.allowed_cidrs.length,
        active_block_count: activeBlockCount,
        ssh_ports: sshPorts,
        log_source: availability.log_source,
        available: availability.available,
        unavailable_reason: availability.reason,
        updated_at: config.updated_at,
      },
    };
  }

  async getAvailability(): Promise<{
    available: boolean;
    reason: string;
    log_source: "journal" | "auth.log" | "unavailable";
  }> {
    const logSource = await sshLogSource.detect();
    if (getRuntimeProfile().deployment_target === "openwrt") {
      return {
        available: false,
        reason: sshSecurityT("openWrtUnsupported"),
        log_source: logSource,
      };
    }
    if (!getRuntimeCapabilities().host_firewall_available) {
      return {
        available: false,
        reason: getCapabilityUnavailableMessage("host_firewall_available"),
        log_source: logSource,
      };
    }
    if (logSource === "unavailable") {
      return {
        available: false,
        reason: sshSecurityT("logSourceUnavailable"),
        log_source: logSource,
      };
    }
    return {
      available: true,
      reason: "",
      log_source: logSource,
    };
  }

  async updateConfig(
    input: SSHSecurityConfigInput,
  ): Promise<SSHSecurityDetails> {
    const previousConfig = await configManager.getSSHSecurityConfig();
    const compiled = await compileSSHSecurityConfig(input, previousConfig);
    if (compiled.config.enabled) {
      const availability = await this.getAvailability();
      if (!availability.available) {
        throw new Error(availability.reason || sshSecurityT("enableUnavailable"));
      }
    }

    await Promise.all([
      configManager.updateSSHSecurityConfig(compiled.config),
      sshSecurityStore.saveRuntimeState(compiled.runtime),
    ]);
    await this.applyConfig(compiled.config, compiled.runtime);
    return this.getDetails();
  }

  async patchEnabled(enabled: boolean): Promise<SSHSecurityDetails> {
    const current = await configManager.getSSHSecurityConfig();
    return this.updateConfig({
      enabled,
      window_minutes: current.window_minutes,
      failed_login_threshold: current.failed_login_threshold,
      block_duration_value: current.block_duration_value,
      block_duration_unit: current.block_duration_unit,
      allowed_regions: current.allowed_regions.map((item) => ({
        province: item.province,
        query_city: item.query_city,
      })),
      custom_cidrs: current.custom_cidrs,
    });
  }

  async syncFromConfigOnBoot(): Promise<void> {
    const [config, runtime] = await Promise.all([
      configManager.getSSHSecurityConfig(),
      sshSecurityStore.getRuntimeState(),
    ]);

    if (!config.enabled) {
      await this.disable(runtime);
      return;
    }

    const availability = await this.getAvailability();
    if (!availability.available) {
      console.warn(`[ssh-security] skipped boot sync: ${availability.reason}`);
      await this.disable(runtime);
      return;
    }

    await this.applyConfig(config, runtime);
  }

  async applyConfig(
    config: SSHSecurityConfig,
    runtime: SSHSecurityRuntimeState,
  ): Promise<void> {
    if (!config.enabled || !runtime.enabled) {
      await this.disable(runtime);
      return;
    }

    await this.start(config, runtime);
  }

  async listLoginLogs(input: {
    page?: unknown;
    limit?: unknown;
    search?: unknown;
    outcome?: unknown;
  }): Promise<SSHLoginLogListResult> {
    const page = normalizePage(input.page);
    const limit = normalizeLimit(input.limit);
    const search = String(input.search ?? "")
      .trim()
      .toLowerCase();
    const outcome =
      input.outcome === "success" || input.outcome === "failure"
        ? input.outcome
        : "";
    const rawEntries = await sshLogSource.queryRecent(
      Math.max(500, page * limit * 5 + limit * 5),
    );

    let entries = rawEntries.filter((entry) => {
      if (outcome && entry.outcome !== outcome) return false;
      if (!search) return true;
      return (
        entry.ip.toLowerCase().includes(search) ||
        entry.username.toLowerCase().includes(search) ||
        entry.raw.toLowerCase().includes(search)
      );
    });

    entries.sort(
      (left, right) =>
        Date.parse(right.happened_at) - Date.parse(left.happened_at),
    );
    entries = coalesceSuccessLoginLogs(entries);
    const total = entries.length;
    const start = (page - 1) * limit;
    entries = entries.slice(start, start + limit);
    await ipLocationService.hydrateIpLocationRecords(entries, (entry) =>
      ipLocationRefs.sshLoginLog(entry.id),
    );

    return {
      items: entries,
      total,
      page,
      limit,
    };
  }

  async listBlocks(input: {
    page?: unknown;
    limit?: unknown;
    search?: unknown;
  }): Promise<SSHSecurityBlockListResult> {
    const page = normalizePage(input.page);
    const limit = normalizeLimit(input.limit);
    const result = await sshSecurityStore.listBlocks({
      page,
      limit,
      search: String(input.search ?? ""),
    });
    await ipLocationService.hydrateIpLocationRecords(result.items, (record) =>
      ipLocationRefs.sshBlocklist(record.ip),
    );
    return {
      items: result.items,
      total: result.total,
      page,
      limit,
    };
  }

  async getBlock(ip: string): Promise<SSHSecurityBlockRecord | null> {
    const record = await sshSecurityStore.getBlock(ip);
    if (!record) return null;
    if (!record.applied || Date.parse(record.expires_at) <= Date.now()) {
      return null;
    }
    await ipLocationService.hydrateIpLocationRecords([record], (item) =>
      ipLocationRefs.sshBlocklist(item.ip),
    );
    return record;
  }

  async removeBlock(
    ip: string,
    reason: "manual" | "expired" | "disabled" = "manual",
  ): Promise<boolean> {
    const record = await sshSecurityStore.getBlock(ip);
    if (!record) return false;
    if (record.applied) {
      const active = (await sshSecurityStore.getActiveBlocks()).filter(
        (item) => item.ip !== record.ip,
      );
      await this.syncFirewallPolicy({ activeBlocks: active });
    }
    await sshSecurityStore.markBlockRemoved(record.ip, reason);
    if (reason === "manual") {
      await sshSecurityStore.clearFailures(record.ip);
    }
    return true;
  }

  async removeBlocks(ips: string[]): Promise<number> {
    let count = 0;
    const uniqueIps = [
      ...new Set(ips.map((ip) => normalizeIp(ip)).filter(Boolean)),
    ];
    for (const ip of uniqueIps) {
      if (await this.removeBlock(ip, "manual")) {
        count += 1;
      }
    }
    return count;
  }

  async syncFirewallBlocks(): Promise<SSHSecurityFirewallSyncResult> {
    const availability = await this.getAvailability();
    if (!availability.available) {
      throw new Error(availability.reason || sshSecurityT("syncFirewallUnavailable"));
    }

    await this.reconcileExpiredBlocks();
    const active = await sshSecurityStore.getActiveBlocks();
    const syncResult = await this.syncFirewallPolicy({
      activeBlocks: active,
      refreshPorts: true,
    });
    const ports = syncResult.ports;
    let synced = 0;
    for (const record of active) {
      await sshSecurityStore.upsertBlock({
        ...record,
        ports,
        applied: true,
        removed_at: null,
        remove_reason: null,
      });
      synced += 1;
    }

    this.scheduleExpiryTimer();
    return {
      cleared: active.length,
      synced,
      active_blocks: active.length,
      allowed_cidrs: syncResult.allowedCidrs,
      ports,
    };
  }

  async clearFirewall(): Promise<SSHSecurityFirewallClearResult> {
    const availability = await this.getAvailability();
    if (!availability.available) {
      throw new Error(availability.reason || sshSecurityT("clearFirewallUnavailable"));
    }

    const active = await sshSecurityStore.getActiveBlocks(0);
    await sshSecurityFirewall.clearSSHPolicy();
    let clearedBlocks = 0;
    for (const record of active) {
      await sshSecurityStore.markBlockRemoved(record.ip, "manual");
      await sshSecurityStore.clearFailures(record.ip);
      clearedBlocks += 1;
    }
    this.scheduleExpiryTimer();
    return { cleared_blocks: clearedBlocks };
  }

  private async syncFirewallPolicy(
    input: {
      runtime?: SSHSecurityRuntimeState;
      activeBlocks?: SSHSecurityBlockRecord[];
      extraBlockedIps?: string[];
      refreshPorts?: boolean;
    } = {},
  ): Promise<{
    allowedCidrs: number;
    blockedIps: number;
    ports: number[];
  }> {
    const runtime =
      input.runtime ??
      this.runtime ??
      (await sshSecurityStore.getRuntimeState());
    const activeBlocks =
      input.activeBlocks ?? (await sshSecurityStore.getActiveBlocks());
    const blockedIps = [
      ...activeBlocks
        .filter((record) => record.applied)
        .map((record) => record.ip),
      ...(input.extraBlockedIps ?? []),
    ];
    const result = await sshSecurityFirewall.syncSSHPolicy({
      allowedCidrs: runtime.enabled ? runtime.allowed_cidrs : [],
      blockedIps,
      refreshPorts: input.refreshPorts,
    });
    return {
      allowedCidrs: result.allowedCidrs.length,
      blockedIps: result.blockedIps.length,
      ports: result.ports,
    };
  }

  private async start(
    config: SSHSecurityConfig,
    runtime: SSHSecurityRuntimeState,
  ): Promise<void> {
    if (this.starting) return;
    this.starting = true;
    try {
      this.stopWatcher();
      this.runtime = runtime;
      this.cidrMatcher =
        runtime.allowed_cidrs.length > 0
          ? buildCIDRMatcher(runtime.allowed_cidrs)
          : null;

      await this.reconcileExpiredBlocks();
      const activeBlocks = await sshSecurityStore.getActiveBlocks();
      const syncResult = await this.syncFirewallPolicy({
        runtime,
        activeBlocks,
      });
      for (const record of activeBlocks) {
        await sshSecurityStore.upsertBlock({
          ...record,
          ports: syncResult.ports,
          applied: true,
        });
      }
      this.scheduleExpiryTimer();

      const handle = await sshLogSource.follow((entry) =>
        this.handleEntry(entry).catch((error) => {
          console.error("[ssh-security] failed to handle log entry:", error);
        }),
      );
      if (!handle) {
        throw new Error(sshSecurityT("logSourceUnavailableShort"));
      }
      this.followHandle = handle;
      console.log(`[ssh-security] watcher started with ${handle.source}`);
      await this.backfillRecentEntries(config);
    } finally {
      this.starting = false;
    }
  }

  private async backfillRecentEntries(
    config: SSHSecurityConfig,
  ): Promise<void> {
    const windowMs = Math.max(1, config.window_minutes) * 60 * 1000;
    const cutoff = Date.now() - windowMs;
    const entries = (await sshLogSource.queryRecent(STARTUP_BACKFILL_LOG_LIMIT))
      .filter((entry) => {
        const happenedAt = Date.parse(entry.happened_at);
        return Number.isFinite(happenedAt) && happenedAt >= cutoff;
      })
      .sort(
        (left, right) =>
          Date.parse(left.happened_at) - Date.parse(right.happened_at),
      );

    let handled = 0;
    for (const entry of entries) {
      try {
        await this.handleEntry(entry);
        handled += 1;
      } catch (error) {
        console.error(
          "[ssh-security] failed to backfill recent log entry:",
          error,
        );
      }
    }

    if (handled > 0) {
      console.log(`[ssh-security] backfilled ${handled} recent log entries`);
    }
  }

  private async disable(
    runtime: SSHSecurityRuntimeState | null,
  ): Promise<void> {
    this.stopWatcher();
    this.clearExpiryTimer();
    this.runtime = runtime
      ? { ...runtime, enabled: false, allowed_cidrs: [] }
      : null;
    this.cidrMatcher = null;

    const active = await sshSecurityStore.getActiveBlocks(0);
    try {
      await sshSecurityFirewall.clearSSHPolicy();
    } catch (error) {
      console.error(
        "[ssh-security] failed to clear disabled SSH chain:",
        error,
      );
    }
    for (const record of active) {
      try {
        await sshSecurityStore.markBlockRemoved(record.ip, "disabled");
      } catch (error) {
        console.error(
          `[ssh-security] failed to mark disabled block ${record.ip}:`,
          error,
        );
      }
    }

    if (runtime) {
      await sshSecurityStore.saveRuntimeState({
        ...runtime,
        enabled: false,
        allowed_cidrs: [],
        updated_at: new Date().toISOString(),
      });
    }
  }

  private stopWatcher(): void {
    if (!this.followHandle) return;
    this.followHandle.stop();
    this.followHandle = null;
  }

  private clearExpiryTimer(): void {
    if (!this.expiryTimer) return;
    clearTimeout(this.expiryTimer);
    this.expiryTimer = null;
  }

  private async handleEntry(entry: SSHLoginLogEntry): Promise<void> {
    if (await sshSecurityStore.isProcessed(entry.id)) return;

    const config = await configManager.getSSHSecurityConfig();
    if (!config.enabled) {
      await sshSecurityStore.markProcessed(entry.id);
      return;
    }

    const ip = normalizeIp(entry.ip);
    if (!ip || isWhitelistExemptIp(ip) || isIP(ip) === 0) {
      await sshSecurityStore.markProcessed(entry.id);
      return;
    }

    const ipLocation = await ipLocationService.registerUsage(ip, [
      ipLocationRefs.sshLoginLog(entry.id),
    ]);
    const entryWithLocation = ipLocation
      ? { ...entry, ipLocation }
      : { ...entry };

    if (entry.outcome === "failure") {
      await this.handleFailure(config, entryWithLocation);
    } else {
      await this.handleSuccess(config, entryWithLocation);
    }

    await sshSecurityStore.markProcessed(entry.id);
  }

  private async handleFailure(
    config: SSHSecurityConfig,
    entry: SSHLoginLogEntry,
  ): Promise<void> {
    const attempts = await sshSecurityStore.addFailure({
      ip: entry.ip,
      id: entry.id,
      happenedAt: entry.happened_at,
      windowMinutes: config.window_minutes,
    });

    await emitSSHLoginFailureEvent({
      ip: entry.ip,
      ipLocation: entry.ipLocation,
      username: entry.username,
      invalidUser: entry.invalid_user,
      authMethod: entry.auth_method,
      port: entry.port,
      attempts,
      windowMinutes: config.window_minutes,
      threshold: config.failed_login_threshold,
      logTime: entry.happened_at,
    });

    if (attempts < config.failed_login_threshold) return;
    if (await sshSecurityStore.isActiveBlocked(entry.ip)) return;

    await this.createBlock({
      config,
      entry,
      reason: "failed_login_threshold",
      failedCount: attempts,
    });
  }

  private async handleSuccess(
    config: SSHSecurityConfig,
    entry: SSHLoginLogEntry,
  ): Promise<void> {
    await emitSSHLoginSuccessEvent({
      ip: entry.ip,
      ipLocation: entry.ipLocation,
      username: entry.username,
      authMethod: entry.auth_method,
      port: entry.port,
      logTime: entry.happened_at,
    });
    await sshSecurityStore.clearFailures(entry.ip);

    if (!this.runtime) {
      this.runtime = await sshSecurityStore.getRuntimeState();
    }
    if (!this.cidrMatcher && this.runtime.allowed_cidrs.length > 0) {
      this.cidrMatcher = buildCIDRMatcher(this.runtime.allowed_cidrs);
    }
    if (!this.cidrMatcher) return;
    if (this.cidrMatcher.contains(entry.ip)) return;
    if (await sshSecurityStore.isActiveBlocked(entry.ip)) return;

    await this.createBlock({
      config,
      entry,
      reason: "cidr_not_allowed",
      failedCount: 0,
    });
  }

  private async createBlock(input: {
    config: SSHSecurityConfig;
    entry: SSHLoginLogEntry;
    reason: SSHSecurityBlockReason;
    failedCount: number;
  }): Promise<void> {
    const { config, entry, reason, failedCount } = input;
    const blockSeconds = sshSecurityDurationToSeconds(config);
    const now = new Date();
    const expiresAt = new Date(now.getTime() + blockSeconds * 1000);
    const ipLocation =
      entry.ipLocation ||
      (await ipLocationService.registerUsage(entry.ip, [
        ipLocationRefs.sshBlocklist(entry.ip),
      ]));

    const syncResult = await this.syncFirewallPolicy({
      extraBlockedIps: [entry.ip],
    });
    const record = await sshSecurityStore.upsertBlock({
      ip: entry.ip,
      ...(ipLocation ? { ipLocation } : {}),
      ports: syncResult.ports,
      blocked_at: now.toISOString(),
      expires_at: expiresAt.toISOString(),
      reason,
      failed_count: failedCount,
      window_minutes: config.window_minutes,
      threshold: config.failed_login_threshold,
      sample_user: entry.username,
      sample_auth_method: entry.auth_method,
      sample_log_time: entry.happened_at,
      applied: true,
      removed_at: null,
      remove_reason: null,
    });

    await emitSSHIPBlockedEvent({
      ip: record.ip,
      ipLocation: record.ipLocation,
      blockedAt: record.blocked_at,
      blockedUntil: record.expires_at,
      blockSeconds,
      reason,
      failedCount,
      windowMinutes: config.window_minutes,
      threshold: config.failed_login_threshold,
      username: entry.username,
    });
    this.scheduleExpiryTimer();
    console.warn(
      `[ssh-security] blocked ${entry.ip} (${blockReasonTags[reason]}) until ${record.expires_at}`,
    );
  }

  private async reconcileExpiredBlocks(): Promise<void> {
    const expired = await sshSecurityStore.getExpiredActiveBlocks();
    for (const record of expired) {
      try {
        await this.removeBlock(record.ip, "expired");
      } catch (error) {
        console.error(
          `[ssh-security] failed to expire block ${record.ip}:`,
          error,
        );
      }
    }
  }

  private scheduleExpiryTimer(): void {
    this.clearExpiryTimer();
    void sshSecurityStore.getActiveBlocks().then((records) => {
      const next = records
        .map((record) => Date.parse(record.expires_at))
        .filter((ts) => Number.isFinite(ts))
        .sort((left, right) => left - right)[0];
      if (!next) return;

      const delay = Math.max(1000, Math.min(next - Date.now(), 2 ** 31 - 1));
      this.expiryTimer = setTimeout(() => {
        this.expiryTimer = null;
        void this.reconcileExpiredBlocks().finally(() => {
          this.scheduleExpiryTimer();
        });
      }, delay);
      this.expiryTimer.unref?.();
    });
  }
}

export const sshSecurityService = new SSHSecurityService();
