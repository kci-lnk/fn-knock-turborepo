import type Redis from 'ioredis';
import { v4 as uuidv4 } from 'uuid';
import { goBackend } from './go-backend';
import { configManager, redis } from "./redis";
import { ipLocationRefs, ipLocationService } from "./ip-location";
import { normalizeIp } from "./ip-normalize";

export interface WhiteListRecord {
  id: string;
  ip: string;
  expireAt: number | null;
  source: 'manual' | 'auto';
  createdAt: number;
  comment?: string;
  status: 'active' | 'expired' | 'deleted';
  ipLocation?: string;
}

const PREFIX = 'fn_knock:whitelist';
const KEYS = {
  RECORDS: `${PREFIX}:records`,
  RECORD_ORDER: `${PREFIX}:record_order`,
  EXPIRY: `${PREFIX}:expiry`,
  IPS: `${PREFIX}:ips`,
  DELETED: `${PREFIX}:deleted`
};

export class IPTablesWhiteListManager {
  private redis: Redis;

  constructor() {
    this.redis = redis;
  }

  private getIPRecordsKey(ip: string) {
    const normalizedIp = normalizeIp(ip) || String(ip || "").trim();
    return `${PREFIX}:ip_records:${normalizedIp}`;
  }

  async getRecordById(id: string): Promise<WhiteListRecord | null> {
    const raw = await this.redis.hget(KEYS.RECORDS, id);
    if (!raw) return null;
    try {
      return JSON.parse(raw) as WhiteListRecord;
    } catch {
      return null;
    }
  }

  private async findRecordsByIPWithScan(ip: string, rebuildIndex: boolean): Promise<WhiteListRecord[]> {
    const normalizedIp = normalizeIp(ip) || String(ip || "").trim();
    const allRecords = await this.redis.hgetall(KEYS.RECORDS);
    const records: WhiteListRecord[] = [];
    const ids: string[] = [];

    for (const [id, raw] of Object.entries(allRecords)) {
      try {
        const record = JSON.parse(raw) as WhiteListRecord;
        if (
          normalizeIp(record.ip || "") === normalizedIp &&
          record.status === 'active'
        ) {
          records.push(record);
          ids.push(id);
        }
      } catch {
        // ignore invalid payload
      }
    }

    records.sort((a, b) => b.createdAt - a.createdAt);
    if (!rebuildIndex) return records;

    const ipKey = this.getIPRecordsKey(normalizedIp);
    const pipeline = this.redis.pipeline();
    pipeline.del(ipKey);
    if (ids.length > 0) {
      pipeline.sadd(ipKey, ...ids);
    }
    await pipeline.exec();
    return records;
  }

  async addWhiteList(record: Omit<WhiteListRecord, 'id' | 'createdAt' | 'status'>): Promise<string> {
    const normalizedIp = normalizeIp(record.ip) || String(record.ip || "").trim();
    await this.removeRecordsByIP(normalizedIp);
    const id = `whitelist:${uuidv4()}`;
    const now = Math.floor(Date.now() / 1000);
    const ipLocationStr = await ipLocationService.getCachedLocation(normalizedIp);
    const fullRecord: WhiteListRecord = {
      ...record,
      ip: normalizedIp,
      id,
      createdAt: now,
      status: 'active',
      ...(ipLocationStr ? { ipLocation: ipLocationStr } : {})
    };

    const ipKey = this.getIPRecordsKey(normalizedIp);
    const pipeline = this.redis.pipeline();
    pipeline.hset(KEYS.RECORDS, id, JSON.stringify(fullRecord));
    pipeline.zadd(KEYS.RECORD_ORDER, now, id);
    pipeline.sadd(KEYS.IPS, normalizedIp);
    pipeline.sadd(ipKey, id);

    if (record.expireAt) {
      pipeline.zadd(KEYS.EXPIRY, record.expireAt, id);
    }

    await pipeline.exec();
    await ipLocationService.registerUsage(normalizedIp, [ipLocationRefs.whitelist(id)]);
    const config = await configManager.getConfig();
    if (config.run_type == 0) {
      await goBackend.allowIP(normalizedIp);
    }
    return id;
  }

  /**
   * Remove a whitelist record by ID
   */
  async removeWhiteList(id: string): Promise<boolean> {
    const recordStr = await this.redis.hget(KEYS.RECORDS, id);
    if (!recordStr) return false;

    const record: WhiteListRecord = JSON.parse(recordStr);
    const ipKey = this.getIPRecordsKey(record.ip);
    const pipeline = this.redis.pipeline();
    pipeline.hdel(KEYS.RECORDS, id);
    pipeline.hdel(KEYS.DELETED, id);
    pipeline.zrem(KEYS.RECORD_ORDER, id);
    pipeline.zrem(KEYS.EXPIRY, id);
    pipeline.srem(ipKey, id);
    await pipeline.exec();

    const remaining = await this.findRecordsByIP(record.ip);
    if (remaining.length === 0) {
      await this.redis.srem(KEYS.IPS, record.ip);
      await this.redis.del(ipKey);
      const config = await configManager.getConfig();
      const runType = config.run_type;
      if (runType === 0) {
        await goBackend.removeIP(record.ip);
      }
    }

    return true;
  }

  /**
   * Update the comment of a record
   */
  async updateComment(id: string, comment: string): Promise<boolean> {
    const recordStr = await this.redis.hget(KEYS.RECORDS, id);
    if (!recordStr) return false;

    const record: WhiteListRecord = JSON.parse(recordStr);
    record.comment = comment;
    await this.redis.hset(KEYS.RECORDS, id, JSON.stringify(record));
    return true;
  }

  /**
   * Get all active whitelist records
   */
  async getAllActiveRecords(): Promise<WhiteListRecord[]> {
    const ids = await this.redis.zrevrange(KEYS.RECORD_ORDER, 0, -1);
    if (ids.length === 0) {
      return this.rebuildRecordOrderIndex();
    }

    const raws = await this.redis.hmget(KEYS.RECORDS, ...ids);
    const activeRecords: WhiteListRecord[] = [];
    const staleIds: string[] = [];
    const maybeExpiredIds: string[] = [];
    const maybeDeletedIps = new Set<string>();

    raws.forEach((raw, index) => {
      const id = ids[index];
      if (!id) return;
      if (!raw) {
        staleIds.push(id);
        maybeExpiredIds.push(id);
        return;
      }
      try {
        const record = JSON.parse(raw) as WhiteListRecord;
        if (record.status === 'active') {
          activeRecords.push(record);
          return;
        }
        staleIds.push(id);
        maybeExpiredIds.push(id);
        maybeDeletedIps.add(record.ip);
      } catch {
        staleIds.push(id);
        maybeExpiredIds.push(id);
      }
    });

    if (staleIds.length > 0) {
      const pipeline = this.redis.pipeline();
      pipeline.zrem(KEYS.RECORD_ORDER, ...staleIds);
      if (maybeExpiredIds.length > 0) {
        pipeline.zrem(KEYS.EXPIRY, ...maybeExpiredIds);
      }
      for (const ip of maybeDeletedIps) {
        const ipKey = this.getIPRecordsKey(ip);
        pipeline.srem(ipKey, ...staleIds);
      }
      await pipeline.exec();
    }

    await ipLocationService.hydrateIpLocationRecords(activeRecords, (record) =>
      ipLocationRefs.whitelist(record.id),
    );
    return activeRecords;
  }

  private async rebuildRecordOrderIndex(): Promise<WhiteListRecord[]> {
    const allRecords = await this.redis.hgetall(KEYS.RECORDS);
    const activeRecords: WhiteListRecord[] = [];

    for (const raw of Object.values(allRecords)) {
      try {
        const record = JSON.parse(raw) as WhiteListRecord;
        if (record.status === 'active') {
          activeRecords.push(record);
        }
      } catch {
        // ignore invalid payload
      }
    }

    activeRecords.sort((a, b) => b.createdAt - a.createdAt);
    if (activeRecords.length > 0) {
      const pipeline = this.redis.pipeline();
      pipeline.del(KEYS.RECORD_ORDER);
      for (const record of activeRecords) {
        pipeline.zadd(KEYS.RECORD_ORDER, record.createdAt, record.id);
      }
      await pipeline.exec();
    }
    await ipLocationService.hydrateIpLocationRecords(activeRecords, (record) =>
      ipLocationRefs.whitelist(record.id),
    );
    return activeRecords;
  }

  /**
   * Check if an IP is whitelisted
   */
  async isIPWhitelisted(ip: string): Promise<boolean> {
    const normalizedIp = normalizeIp(ip) || String(ip || "").trim();
    if (!normalizedIp) return false;
    return await this.redis.sismember(KEYS.IPS, normalizedIp) === 1;
  }

  /**
   * Check if an IP has at least one valid, active record
   */
  async hasValidIP(ip: string): Promise<boolean> {
    const normalizedIp = normalizeIp(ip) || String(ip || "").trim();
    if (!normalizedIp) return false;

    const isMember = await this.redis.sismember(KEYS.IPS, normalizedIp) === 1;
    const records = await this.findRecordsByIP(normalizedIp);
    if (!isMember && records.length === 0) return false;

    const now = Math.floor(Date.now() / 1000);
    return records.some(r => !r.expireAt || r.expireAt > now);
  }

  private async findRecordsByIP(ip: string): Promise<WhiteListRecord[]> {
    const normalizedIp = normalizeIp(ip) || String(ip || "").trim();
    if (!normalizedIp) return [];

    const ipKey = this.getIPRecordsKey(normalizedIp);
    const ids = await this.redis.smembers(ipKey);
    if (ids.length === 0) {
      return this.findRecordsByIPWithScan(normalizedIp, true);
    }

    const raws = await this.redis.hmget(KEYS.RECORDS, ...ids);
    const records: WhiteListRecord[] = [];
    const removeFromSetOnly: string[] = [];
    const removeFromAllIndexes: string[] = [];

    raws.forEach((raw, index) => {
      const id = ids[index];
      if (!id) return;
      if (!raw) {
        removeFromAllIndexes.push(id);
        return;
      }
      try {
        const record = JSON.parse(raw) as WhiteListRecord;
        if (normalizeIp(record.ip || "") !== normalizedIp) {
          removeFromSetOnly.push(id);
          return;
        }
        if (record.status !== 'active') {
          removeFromAllIndexes.push(id);
          return;
        }
        records.push(record);
      } catch {
        removeFromAllIndexes.push(id);
      }
    });

    if (removeFromSetOnly.length > 0 || removeFromAllIndexes.length > 0) {
      const pipeline = this.redis.pipeline();
      if (removeFromSetOnly.length > 0) {
        pipeline.srem(ipKey, ...removeFromSetOnly);
      }
      if (removeFromAllIndexes.length > 0) {
        pipeline.srem(ipKey, ...removeFromAllIndexes);
        pipeline.zrem(KEYS.RECORD_ORDER, ...removeFromAllIndexes);
        pipeline.zrem(KEYS.EXPIRY, ...removeFromAllIndexes);
      }
      await pipeline.exec();
    }

    if (records.length === 0) {
      return this.findRecordsByIPWithScan(normalizedIp, true);
    }

    records.sort((a, b) => b.createdAt - a.createdAt);
    return records;
  }

  async getActiveRecordsByIP(ip: string, source?: 'manual' | 'auto'): Promise<WhiteListRecord[]> {
    const records = await this.findRecordsByIP(ip);
    const now = Math.floor(Date.now() / 1000);
    return records.filter((record) => {
      if (record.status !== 'active') return false;
      if (record.expireAt && record.expireAt <= now) return false;
      if (source && record.source !== source) return false;
      return true;
    });
  }

  async getLatestActiveRecordByIP(ip: string, source?: 'manual' | 'auto'): Promise<WhiteListRecord | null> {
    const records = await this.getActiveRecordsByIP(ip, source);
    return records[0] || null;
  }

  async moveRecordToIP(id: string, newIp: string): Promise<WhiteListRecord | null> {
    const record = await this.getRecordById(id);
    if (!record || record.status !== 'active') return null;

    const now = Math.floor(Date.now() / 1000);
    if (record.expireAt && record.expireAt <= now) return null;

    const oldIp = normalizeIp(record.ip) || record.ip;
    const normalizedNewIp = normalizeIp(newIp) || String(newIp || "").trim();
    if (!normalizedNewIp) return null;

    if (oldIp === normalizedNewIp) {
      return record;
    }

    const ipLocationStr = await ipLocationService.getCachedLocation(normalizedNewIp);

    const nextRecord: WhiteListRecord = {
      ...record,
      ip: normalizedNewIp,
      ...(ipLocationStr ? { ipLocation: ipLocationStr } : {}),
    };

    const oldIpKey = this.getIPRecordsKey(oldIp);
    const newIpKey = this.getIPRecordsKey(normalizedNewIp);
    const pipeline = this.redis.pipeline();
    pipeline.hset(KEYS.RECORDS, id, JSON.stringify(nextRecord));
    pipeline.srem(oldIpKey, id);
    pipeline.sadd(newIpKey, id);
    pipeline.sadd(KEYS.IPS, normalizedNewIp);
    await pipeline.exec();
    await ipLocationService.registerUsage(normalizedNewIp, [ipLocationRefs.whitelist(id)]);

    const config = await configManager.getConfig();
    if (config.run_type === 0) {
      await goBackend.allowIP(normalizedNewIp);
    }

    const remainingOldRecords = await this.findRecordsByIP(oldIp);
    if (remainingOldRecords.length === 0) {
      await this.redis.srem(KEYS.IPS, oldIp);
      await this.redis.del(oldIpKey);
      if (config.run_type === 0) {
        await goBackend.removeIP(oldIp);
      }
    }

    return nextRecord;
  }

  /**
   * Remove whitelist records by IP (optionally filtered by source)
   */
  async removeRecordsByIP(ip: string, source?: 'manual' | 'auto'): Promise<boolean> {
    const normalizedIp = normalizeIp(ip) || String(ip || "").trim();
    const records = await this.findRecordsByIP(normalizedIp);
    let removed = false;
    for (const record of records) {
      if (!source || record.source === source) {
        const res = await this.removeWhiteList(record.id);
        if (res) removed = true;
      }
    }
    return removed;
  }

  async findExpiredRecords(): Promise<WhiteListRecord[]> {
    const now = Math.floor(Date.now() / 1000);
    const expiredIds = await this.redis.zrangebyscore(KEYS.EXPIRY, 0, now);
    if (expiredIds.length === 0) return [];

    const raws = await this.redis.hmget(KEYS.RECORDS, ...expiredIds);
    const records: WhiteListRecord[] = [];
    const staleIds: string[] = [];

    raws.forEach((raw, index) => {
      const id = expiredIds[index];
      if (!id) return;
      if (!raw) {
        staleIds.push(id);
        return;
      }
      try {
        const record = JSON.parse(raw) as WhiteListRecord;
        if (record.status !== 'active') {
          staleIds.push(id);
          return;
        }
        records.push(record);
      } catch {
        staleIds.push(id);
      }
    });

    if (staleIds.length > 0) {
      await this.redis.zrem(KEYS.EXPIRY, ...staleIds);
    }
    return records;
  }

  async processExpiredRecords(): Promise<void> {
    try {
      const expiredRecords = await this.findExpiredRecords();
      if (expiredRecords.length === 0) return;

      const touchedIps = new Set<string>();
      const pipeline = this.redis.pipeline();

      for (const record of expiredRecords) {
        record.status = 'expired';
        touchedIps.add(record.ip);
        const ipKey = this.getIPRecordsKey(record.ip);
        pipeline.hset(KEYS.RECORDS, record.id, JSON.stringify(record));
        pipeline.zrem(KEYS.EXPIRY, record.id);
        pipeline.zrem(KEYS.RECORD_ORDER, record.id);
        pipeline.srem(ipKey, record.id);
      }

      await pipeline.exec();
      const config = await configManager.getConfig();
      for (const ip of touchedIps) {
        const active = await this.findRecordsByIP(ip);
        if (active.length > 0) continue;
        const ipKey = this.getIPRecordsKey(ip);
        await this.redis.srem(KEYS.IPS, ip);
        await this.redis.del(ipKey);
        if (config.run_type == 0) {
          await goBackend.removeIP(ip);
        }
      }
    } catch (error) {
      console.error('Error processing expired records:', error);
    }
  }

  async cleanup(): Promise<void> {
    return;
  }
}

export const whitelistManager = new IPTablesWhiteListManager();
