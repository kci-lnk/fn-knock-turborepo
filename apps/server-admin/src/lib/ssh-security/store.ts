import { normalizeIp } from "../ip-normalize";
import { redis } from "../redis";
import type {
  SSHSecurityBlockRecord,
  SSHSecurityBlockRemoveReason,
  SSHSecurityRuntimeState,
} from "./types";
import { normalizeSSHSecurityRuntimeState } from "./config";
import { tDefault } from "../i18n";

const PREFIX = "fn_knock:ssh_security";
const PROCESSED_TTL_SECONDS = 7 * 24 * 3600;
const BLOCK_RECORD_RETENTION_SECONDS = 90 * 24 * 3600;
const MAX_BLOCK_RECORD_TTL_SECONDS = (365 + 90) * 24 * 3600;
const sshSecurityT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.sshSecurity.${key}`, params);

const KEYS = {
  runtime: `${PREFIX}:runtime`,
  blocksIndex: `${PREFIX}:blocks:index`,
  blockData: (ip: string) => `${PREFIX}:blocks:data:${ip}`,
  blockDataPattern: `${PREFIX}:blocks:data:*`,
  failures: (ip: string) => `${PREFIX}:failures:${ip}`,
  processed: (fingerprint: string) => `${PREFIX}:processed:${fingerprint}`,
};

const normalizeDateString = (value: unknown, fallback = ""): string => {
  const parsed = Date.parse(String(value ?? ""));
  if (Number.isFinite(parsed)) return new Date(parsed).toISOString();
  return fallback;
};

const normalizePorts = (value: unknown): number[] => {
  if (!Array.isArray(value)) return [];
  return [
    ...new Set(
      value
        .map((item) => Number.parseInt(String(item ?? ""), 10))
        .filter((port) => Number.isInteger(port) && port > 0 && port <= 65535),
    ),
  ].sort((left, right) => left - right);
};

const normalizeBlockRecord = (
  value: unknown,
): SSHSecurityBlockRecord | null => {
  if (!value || typeof value !== "object") return null;
  const raw = value as Partial<SSHSecurityBlockRecord>;
  const ip = normalizeIp(raw.ip);
  const blockedAt = normalizeDateString(raw.blocked_at);
  const expiresAt = normalizeDateString(raw.expires_at);
  const ports = normalizePorts(raw.ports);
  if (!ip || !blockedAt || !expiresAt) return null;

  return {
    ip,
    ...(typeof raw.ipLocation === "string" && raw.ipLocation.trim()
      ? { ipLocation: raw.ipLocation.trim() }
      : {}),
    ...(ports.length > 0 ? { ports } : {}),
    blocked_at: blockedAt,
    expires_at: expiresAt,
    reason:
      raw.reason === "cidr_not_allowed"
        ? "cidr_not_allowed"
        : "failed_login_threshold",
    failed_count: Math.max(
      0,
      Number.parseInt(String(raw.failed_count ?? 0), 10) || 0,
    ),
    window_minutes: Math.max(
      0,
      Number.parseInt(String(raw.window_minutes ?? 0), 10) || 0,
    ),
    threshold: Math.max(
      0,
      Number.parseInt(String(raw.threshold ?? 0), 10) || 0,
    ),
    ...(typeof raw.sample_user === "string" && raw.sample_user.trim()
      ? { sample_user: raw.sample_user.trim() }
      : {}),
    ...(typeof raw.sample_auth_method === "string" &&
    raw.sample_auth_method.trim()
      ? { sample_auth_method: raw.sample_auth_method.trim() }
      : {}),
    ...(typeof raw.sample_log_time === "string" && raw.sample_log_time.trim()
      ? { sample_log_time: normalizeDateString(raw.sample_log_time) }
      : {}),
    applied: raw.applied === true,
    removed_at: raw.removed_at ? normalizeDateString(raw.removed_at, "") : null,
    remove_reason:
      raw.remove_reason === "manual" ||
      raw.remove_reason === "expired" ||
      raw.remove_reason === "disabled"
        ? raw.remove_reason
        : null,
  };
};

const isAppliedBlockRecord = (record: SSHSecurityBlockRecord): boolean =>
  record.applied === true;

const isActiveBlockRecord = (
  record: SSHSecurityBlockRecord,
  nowMs = Date.now(),
): boolean => record.applied && Date.parse(record.expires_at) > nowMs;

const isExpiredAppliedBlockRecord = (
  record: SSHSecurityBlockRecord,
  nowMs = Date.now(),
): boolean => record.applied && Date.parse(record.expires_at) <= nowMs;

const resolveBlockRecordTtlSeconds = (
  record: SSHSecurityBlockRecord,
  nowMs = Date.now(),
): number => {
  if (!record.applied) return BLOCK_RECORD_RETENTION_SECONDS;

  const expiresAtMs = Date.parse(record.expires_at);
  if (!Number.isFinite(expiresAtMs)) return BLOCK_RECORD_RETENTION_SECONDS;

  const secondsUntilExpiry = Math.max(
    0,
    Math.ceil((expiresAtMs - nowMs) / 1000),
  );
  return Math.min(
    MAX_BLOCK_RECORD_TTL_SECONDS,
    Math.max(
      BLOCK_RECORD_RETENTION_SECONDS,
      secondsUntilExpiry + BLOCK_RECORD_RETENTION_SECONDS,
    ),
  );
};

const parseBlockRecord = (
  raw: string | null,
): SSHSecurityBlockRecord | null => {
  if (!raw) return null;
  try {
    return normalizeBlockRecord(JSON.parse(raw));
  } catch {
    return null;
  }
};

export class SSHSecurityStore {
  async getRuntimeState(): Promise<SSHSecurityRuntimeState> {
    const raw = await redis.get(KEYS.runtime);
    if (!raw) return normalizeSSHSecurityRuntimeState();
    try {
      return normalizeSSHSecurityRuntimeState(JSON.parse(raw));
    } catch {
      return normalizeSSHSecurityRuntimeState();
    }
  }

  async saveRuntimeState(
    runtime: SSHSecurityRuntimeState,
  ): Promise<SSHSecurityRuntimeState> {
    const next = normalizeSSHSecurityRuntimeState(runtime);
    await redis.set(KEYS.runtime, JSON.stringify(next));
    return next;
  }

  async isProcessed(id: string): Promise<boolean> {
    if (!id) return false;
    return (await redis.exists(KEYS.processed(id))) > 0;
  }

  async markProcessed(id: string): Promise<void> {
    if (!id) return;
    await redis.set(KEYS.processed(id), "1", "EX", PROCESSED_TTL_SECONDS);
  }

  async addFailure(input: {
    ip: string;
    id: string;
    happenedAt: string;
    windowMinutes: number;
  }): Promise<number> {
    const ip = normalizeIp(input.ip);
    if (!ip) return 0;
    const happenedAt = Date.parse(input.happenedAt);
    const score = Number.isFinite(happenedAt) ? happenedAt : Date.now();
    const windowMs = Math.max(1, input.windowMinutes) * 60 * 1000;
    const key = KEYS.failures(ip);

    const pipeline = redis.pipeline();
    pipeline.zadd(key, score, input.id || `${score}`);
    pipeline.zremrangebyscore(key, 0, score - windowMs);
    pipeline.expire(key, Math.ceil(windowMs / 1000) + 3600);
    pipeline.zcard(key);
    const result = await pipeline.exec();
    const count = Number(result?.[3]?.[1] ?? 0);
    return Number.isFinite(count) ? count : 0;
  }

  async clearFailures(ip: string): Promise<void> {
    const normalizedIp = normalizeIp(ip);
    if (!normalizedIp) return;
    await redis.del(KEYS.failures(normalizedIp));
  }

  async getBlock(ip: string): Promise<SSHSecurityBlockRecord | null> {
    const normalizedIp = normalizeIp(ip);
    if (!normalizedIp) return null;
    return parseBlockRecord(await redis.get(KEYS.blockData(normalizedIp)));
  }

  async isActiveBlocked(ip: string, nowMs = Date.now()): Promise<boolean> {
    const record = await this.getBlock(ip);
    return record ? isActiveBlockRecord(record, nowMs) : false;
  }

  async upsertBlock(
    record: SSHSecurityBlockRecord,
  ): Promise<SSHSecurityBlockRecord> {
    const normalized = normalizeBlockRecord(record);
    if (!normalized) {
      throw new Error(sshSecurityT("blockRecordInvalid"));
    }

    const expiresAtMs = Date.parse(normalized.expires_at);
    const pipeline = redis.pipeline();
    pipeline.set(
      KEYS.blockData(normalized.ip),
      JSON.stringify(normalized),
      "EX",
      resolveBlockRecordTtlSeconds(normalized),
    );
    if (normalized.applied && Number.isFinite(expiresAtMs)) {
      pipeline.zadd(KEYS.blocksIndex, expiresAtMs, normalized.ip);
    } else {
      pipeline.zrem(KEYS.blocksIndex, normalized.ip);
    }
    await pipeline.exec();
    return normalized;
  }

  async markBlockRemoved(
    ip: string,
    reason: SSHSecurityBlockRemoveReason,
    removedAt = new Date().toISOString(),
  ): Promise<SSHSecurityBlockRecord | null> {
    const record = await this.getBlock(ip);
    if (!record) return null;

    return this.upsertBlock({
      ...record,
      applied: false,
      removed_at: removedAt,
      remove_reason: reason,
    });
  }

  async getActiveBlocks(nowMs = Date.now()): Promise<SSHSecurityBlockRecord[]> {
    const ips = await redis.zrangebyscore(
      KEYS.blocksIndex,
      `(${nowMs}`,
      "+inf",
    );
    return this.getBlocksByIps(ips, { state: "active", nowMs });
  }

  async getExpiredActiveBlocks(
    nowMs = Date.now(),
  ): Promise<SSHSecurityBlockRecord[]> {
    const ips = await redis.zrangebyscore(KEYS.blocksIndex, 0, nowMs);
    return this.getBlocksByIps(ips, { state: "expired", nowMs });
  }

  async activeBlockCount(): Promise<number> {
    return (await this.getActiveBlocks()).length;
  }

  async removeFromActiveIndex(ip: string): Promise<void> {
    const normalizedIp = normalizeIp(ip);
    if (!normalizedIp) return;
    await redis.zrem(KEYS.blocksIndex, normalizedIp);
  }

  async listBlocks(input: {
    page: number;
    limit: number;
    search?: string;
  }): Promise<{ items: SSHSecurityBlockRecord[]; total: number }> {
    const nowMs = Date.now();
    const records = (await this.scanBlockRecords()).filter((record) =>
      isActiveBlockRecord(record, nowMs),
    );
    const query = String(input.search ?? "")
      .trim()
      .toLowerCase();
    const filtered = records.filter((record) => {
      if (!query) return true;
      return (
        record.ip.toLowerCase().includes(query) ||
        String(record.ipLocation ?? "")
          .toLowerCase()
          .includes(query) ||
        String(record.sample_user ?? "")
          .toLowerCase()
          .includes(query)
      );
    });
    filtered.sort(
      (left, right) =>
        Date.parse(right.blocked_at) - Date.parse(left.blocked_at),
    );

    const page = Math.max(1, input.page);
    const limit = Math.max(1, Math.min(100, input.limit));
    const start = (page - 1) * limit;
    return {
      items: filtered.slice(start, start + limit),
      total: filtered.length,
    };
  }

  private async getBlocksByIps(
    ips: string[],
    options: {
      state: "active" | "expired" | "applied";
      nowMs?: number;
    },
  ): Promise<SSHSecurityBlockRecord[]> {
    const normalizedIps = [
      ...new Set(ips.map((ip) => normalizeIp(ip)).filter(Boolean)),
    ];
    if (normalizedIps.length === 0) return [];

    const raws = await redis.mget(
      normalizedIps.map((ip) => KEYS.blockData(ip)),
    );
    const nowMs = options.nowMs ?? Date.now();
    const records: SSHSecurityBlockRecord[] = [];
    const staleIps: string[] = [];
    const reindexRecords: SSHSecurityBlockRecord[] = [];

    raws.forEach((raw, index) => {
      const ip = normalizedIps[index];
      const record = parseBlockRecord(raw);
      if (!record) {
        if (ip) staleIps.push(ip);
        return;
      }
      if (!isAppliedBlockRecord(record)) {
        staleIps.push(record.ip);
        return;
      }

      if (options.state === "active" && isActiveBlockRecord(record, nowMs)) {
        records.push(record);
        return;
      }
      if (
        options.state === "active" &&
        isExpiredAppliedBlockRecord(record, nowMs)
      ) {
        reindexRecords.push(record);
        return;
      }
      if (
        options.state === "expired" &&
        isExpiredAppliedBlockRecord(record, nowMs)
      ) {
        records.push(record);
        return;
      }
      if (options.state === "expired" && isActiveBlockRecord(record, nowMs)) {
        reindexRecords.push(record);
        return;
      }
      if (options.state === "applied") {
        records.push(record);
        return;
      }
    });

    if (staleIps.length > 0 || reindexRecords.length > 0) {
      const pipeline = redis.pipeline();
      if (staleIps.length > 0) {
        pipeline.zrem(KEYS.blocksIndex, ...staleIps);
      }
      for (const record of reindexRecords) {
        const expiresAtMs = Date.parse(record.expires_at);
        if (Number.isFinite(expiresAtMs)) {
          pipeline.zadd(KEYS.blocksIndex, expiresAtMs, record.ip);
        }
      }
      await pipeline.exec();
    }

    return records;
  }

  private async scanBlockRecords(): Promise<SSHSecurityBlockRecord[]> {
    let cursor = "0";
    const keys: string[] = [];

    do {
      const [nextCursor, batch] = await redis.scan(
        cursor,
        "MATCH",
        KEYS.blockDataPattern,
        "COUNT",
        "100",
      );
      cursor = nextCursor;
      keys.push(...batch);
    } while (cursor !== "0");

    if (keys.length === 0) return [];
    const raws = await redis.mget(keys);
    return raws
      .map(parseBlockRecord)
      .filter((record): record is SSHSecurityBlockRecord => record !== null);
  }
}

export const sshSecurityStore = new SSHSecurityStore();
