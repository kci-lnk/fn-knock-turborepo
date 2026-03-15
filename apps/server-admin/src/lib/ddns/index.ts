import { redis } from "../redis";
import { DEFAULT_REDIS_LOG_BUFFER_MAX_LEN, RedisLogBuffer } from "../redis-log-buffer";
import type { DDNSLastCheck, DDNSLastIP, DDNSLogEntry, DDNSProviderDefinition, DDNSProviderField, DDNSStatus, DDNSUpdateResult } from "./types";
import { providerDefinitions, providerUpdaters } from "./providers";
import { runWithRetry } from "./retry";

const KEYS = {
  enabled: "fn_knock:ddns:enabled",
  provider: "fn_knock:ddns:provider",
  configPrefix: "fn_knock:ddns:config:",
  lastIP: "fn_knock:ddns:last_ip",
  lastCheck: "fn_knock:ddns:last_check",
  logs: "fn_knock:ddns:logs",
  logSeq: "fn_knock:ddns:logs:seq",
} as const;

const LOG_TTL = 7 * 24 * 3600;
const ddnsLogBuffer = new RedisLogBuffer(redis, {
  key: KEYS.logs,
  ttlSeconds: LOG_TTL,
  maxLen: DEFAULT_REDIS_LOG_BUFFER_MAX_LEN,
  seqKey: KEYS.logSeq,
});

export class DDNSManager {
  getProviders(): DDNSProviderDefinition[] {
    return providerDefinitions;
  }

  getProviderFields(name: string): DDNSProviderField[] | null {
    const p = providerDefinitions.find((d) => d.name === name);
    return p ? p.fields : null;
  }

  async isEnabled(): Promise<boolean> {
    const v = await redis.get(KEYS.enabled);
    return v === "true";
  }

  async setEnabled(enabled: boolean): Promise<void> {
    await redis.set(KEYS.enabled, enabled ? "true" : "false");
  }

  async getProvider(): Promise<string | null> {
    return redis.get(KEYS.provider);
  }

  async setProvider(name: string): Promise<void> {
    if (!providerDefinitions.find((d) => d.name === name)) {
      throw new Error(`未知的 DDNS 提供商: ${name}`);
    }
    await redis.set(KEYS.provider, name);
  }

  async getConfig(providerName: string): Promise<Record<string, string>> {
    const data = await redis.hgetall(KEYS.configPrefix + providerName);
    return data || {};
  }

  async saveConfig(providerName: string, config: Record<string, string>): Promise<void> {
    const key = KEYS.configPrefix + providerName;
    await redis.del(key);
    if (Object.keys(config).length > 0) {
      await redis.hmset(key, config);
    }
  }

  async getLastIP(): Promise<DDNSLastIP> {
    const data = await redis.hgetall(KEYS.lastIP);
    return {
      ipv4: data?.ipv4 || null,
      ipv6: data?.ipv6 || null,
      updated_at: data?.updated_at || null,
    };
  }

  async setLastIP(ipv4: string | null, ipv6: string | null): Promise<void> {
    const now = new Date().toISOString();
    const map: Record<string, string> = { updated_at: now };
    if (ipv4) map.ipv4 = ipv4;
    if (ipv6) map.ipv6 = ipv6;
    await redis.del(KEYS.lastIP);
    await redis.hmset(KEYS.lastIP, map);
  }

  async getLastCheck(): Promise<DDNSLastCheck> {
    const data = await redis.hgetall(KEYS.lastCheck);
    const rawOutcome = data?.outcome;
    const outcome = rawOutcome === "updated" || rawOutcome === "noop" || rawOutcome === "skipped" || rawOutcome === "error"
      ? rawOutcome
      : null;

    return {
      checked_at: data?.checked_at || null,
      outcome,
      message: data?.message || null,
    };
  }

  async setLastCheck(
    outcome: NonNullable<DDNSLastCheck["outcome"]>,
    message: string,
  ): Promise<void> {
    const map = {
      checked_at: new Date().toISOString(),
      outcome,
      message,
    };
    await redis.del(KEYS.lastCheck);
    await redis.hmset(KEYS.lastCheck, map);
  }

  async getStatus(): Promise<DDNSStatus> {
    const [enabled, provider, lastIP, lastCheck] = await Promise.all([
      this.isEnabled(),
      this.getProvider(),
      this.getLastIP(),
      this.getLastCheck(),
    ]);
    return { enabled, provider, lastIP, lastCheck };
  }

  async appendLog(level: DDNSLogEntry["level"], message: string): Promise<void> {
    const entry: DDNSLogEntry = { time: new Date().toISOString(), level, message };
    await ddnsLogBuffer.append([JSON.stringify(entry)]);
  }

  async getLogs(limit: number = 200): Promise<DDNSLogEntry[]> {
    const raw = await ddnsLogBuffer.list(limit);
    return raw.map((s) => {
      try { return JSON.parse(s); } catch { return { time: "", level: "info", message: s }; }
    });
  }

  async clearLogs(): Promise<void> {
    await ddnsLogBuffer.clear();
  }

  async executeUpdate(ipv4: string | null, ipv6: string | null): Promise<DDNSUpdateResult> {
    const providerName = await this.getProvider();
    if (!providerName) {
      return { success: false, message: "未选择 DDNS 提供商" };
    }

    const updater = providerUpdaters[providerName];
    if (!updater) {
      return { success: false, message: `未知的提供商: ${providerName}` };
    }

    const config = await this.getConfig(providerName);
    const retryCount = Number(process.env.DDNS_RETRY_COUNT || "1");
    const maxAttempts = Math.max(1, retryCount + 1);
    const delayMs = Number(process.env.DDNS_RETRY_DELAY_MS || "600");

    try {
      return await runWithRetry(() => updater(config, ipv4, ipv6), { maxAttempts, delayMs });
    } catch (e: any) {
      const message = e?.message || String(e);
      return { success: false, message };
    }
  }

  async isConfigComplete(): Promise<boolean> {
    const providerName = await this.getProvider();
    if (!providerName) return false;

    const def = providerDefinitions.find((d) => d.name === providerName);
    if (!def) return false;

    const config = await this.getConfig(providerName);
    const requiredFields = def.fields.filter((f) => f.required !== false);
    return requiredFields.every((f) => !!config[f.key]);
  }
}

export const ddnsManager = new DDNSManager();
export { ddnsLogBuffer };

export type {
  DDNSLastCheck,
  DDNSLastIP,
  DDNSLogEntry,
  DDNSProviderDefinition,
  DDNSProviderField,
  DDNSStatus,
  DDNSUpdateResult,
} from "./types";
