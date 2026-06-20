import type Redis from "ioredis";
import { ipLocationRefs, ipLocationService } from "../ip-location";
import { normalizeIp } from "../ip-normalize";
import { KEYS, getIPRecordsKey } from "./keys";
import {
  deserializeRecord,
  getCnameResolvedTargets,
  getConcreteIPTargets,
  isCIDRRecord,
  isCNAMERecord,
  isIPRecord,
  sortRecordsByCreatedAtDesc,
  type WhiteListRecord,
} from "./record";

export class WhitelistRecordIndex {
  constructor(private readonly redis: Redis) {}

  async getAllActiveRecords(
    source?: "manual" | "auto",
  ): Promise<WhiteListRecord[]> {
    const ids = await this.redis.zrevrange(KEYS.RECORD_ORDER, 0, -1);
    if (ids.length === 0) {
      const rebuilt = await this.rebuildRecordOrderIndex();
      return source
        ? rebuilt.filter((record) => record.source === source)
        : rebuilt;
    }

    const raws = await this.redis.hmget(KEYS.RECORDS, ...ids);
    const activeRecords: WhiteListRecord[] = [];
    const staleIds: string[] = [];
    const staleIPTargets = new Set<string>();

    raws.forEach((raw, index) => {
      const id = ids[index];
      if (!id) return;
      if (!raw) {
        staleIds.push(id);
        return;
      }

      const record = deserializeRecord(raw);
      if (!record) {
        staleIds.push(id);
        return;
      }
      if (record.status !== "active") {
        staleIds.push(id);
        for (const target of getConcreteIPTargets(record)) {
          staleIPTargets.add(target);
        }
        return;
      }

      activeRecords.push(record);
    });

    if (staleIds.length > 0) {
      const pipeline = this.redis.pipeline();
      pipeline.zrem(KEYS.RECORD_ORDER, ...staleIds);
      pipeline.zrem(KEYS.EXPIRY, ...staleIds);
      pipeline.srem(KEYS.CIDR_RECORDS, ...staleIds);
      for (const ip of staleIPTargets) {
        pipeline.srem(getIPRecordsKey(ip), ...staleIds);
      }
      await pipeline.exec();
    }

    await ipLocationService.hydrateIpLocationRecords(activeRecords, (record) =>
      ipLocationRefs.whitelist(record.id),
    );
    const sorted = sortRecordsByCreatedAtDesc(activeRecords);
    return source
      ? sorted.filter((record) => record.source === source)
      : sorted;
  }

  async findExactIPRecords(ip: string): Promise<WhiteListRecord[]> {
    const normalizedIp = normalizeIp(ip) || String(ip || "").trim();
    if (!normalizedIp) return [];

    const ipKey = getIPRecordsKey(normalizedIp);
    const ids = await this.redis.smembers(ipKey);
    if (ids.length === 0) {
      return this.findExactIPRecordsWithScan(normalizedIp, true);
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

      const record = deserializeRecord(raw);
      if (!record) {
        removeFromAllIndexes.push(id);
        return;
      }
      if (
        !(
          (isIPRecord(record) &&
            normalizeIp(record.ip || "") === normalizedIp) ||
          (isCNAMERecord(record) &&
            getCnameResolvedTargets(record).includes(normalizedIp))
        )
      ) {
        if (record.status === "active") {
          removeFromSetOnly.push(id);
        } else {
          removeFromAllIndexes.push(id);
        }
        return;
      }
      if (record.status !== "active") {
        removeFromAllIndexes.push(id);
        return;
      }

      records.push(record);
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
      return this.findExactIPRecordsWithScan(normalizedIp, true);
    }

    return sortRecordsByCreatedAtDesc(records);
  }

  async getAllActiveCIDRRecords(): Promise<WhiteListRecord[]> {
    const ids = await this.redis.smembers(KEYS.CIDR_RECORDS);
    if (ids.length === 0) {
      return this.findAllActiveCIDRRecordsWithScan(true);
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

      const record = deserializeRecord(raw);
      if (!record) {
        removeFromAllIndexes.push(id);
        return;
      }
      if (!isCIDRRecord(record)) {
        if (record.status === "active") {
          removeFromSetOnly.push(id);
        } else {
          removeFromAllIndexes.push(id);
        }
        return;
      }
      if (record.status !== "active") {
        removeFromAllIndexes.push(id);
        return;
      }

      records.push(record);
    });

    if (removeFromSetOnly.length > 0 || removeFromAllIndexes.length > 0) {
      const pipeline = this.redis.pipeline();
      if (removeFromSetOnly.length > 0) {
        pipeline.srem(KEYS.CIDR_RECORDS, ...removeFromSetOnly);
      }
      if (removeFromAllIndexes.length > 0) {
        pipeline.srem(KEYS.CIDR_RECORDS, ...removeFromAllIndexes);
        pipeline.zrem(KEYS.RECORD_ORDER, ...removeFromAllIndexes);
        pipeline.zrem(KEYS.EXPIRY, ...removeFromAllIndexes);
      }
      await pipeline.exec();
    }

    if (records.length === 0) {
      return this.findAllActiveCIDRRecordsWithScan(true);
    }

    return sortRecordsByCreatedAtDesc(records);
  }

  async rebuildRecordOrderIndex(): Promise<WhiteListRecord[]> {
    const allRecords = await this.redis.hgetall(KEYS.RECORDS);
    const existingIndexedIps = await this.redis.smembers(KEYS.IPS);
    const activeRecords: WhiteListRecord[] = [];
    const ipRecordIds = new Map<string, string[]>();
    const cidrRecordIds: string[] = [];

    for (const raw of Object.values(allRecords)) {
      const record = deserializeRecord(raw);
      if (!record || record.status !== "active") {
        continue;
      }

      activeRecords.push(record);
      if (isCIDRRecord(record)) {
        cidrRecordIds.push(record.id);
        continue;
      }

      for (const target of getConcreteIPTargets(record)) {
        const ids = ipRecordIds.get(target) ?? [];
        ids.push(record.id);
        ipRecordIds.set(target, ids);
      }
    }

    sortRecordsByCreatedAtDesc(activeRecords);
    const pipeline = this.redis.pipeline();
    pipeline.del(KEYS.RECORD_ORDER);
    pipeline.del(KEYS.EXPIRY);
    pipeline.del(KEYS.IPS);
    pipeline.del(KEYS.CIDR_RECORDS);

    for (const ip of new Set([...existingIndexedIps, ...ipRecordIds.keys()])) {
      pipeline.del(getIPRecordsKey(ip));
    }

    for (const record of activeRecords) {
      pipeline.zadd(KEYS.RECORD_ORDER, record.createdAt, record.id);
      if (record.expireAt) {
        pipeline.zadd(KEYS.EXPIRY, record.expireAt, record.id);
      }
    }

    for (const [ip, ids] of ipRecordIds.entries()) {
      pipeline.sadd(KEYS.IPS, ip);
      pipeline.sadd(getIPRecordsKey(ip), ...ids);
    }
    if (cidrRecordIds.length > 0) {
      pipeline.sadd(KEYS.CIDR_RECORDS, ...cidrRecordIds);
    }

    await pipeline.exec();
    await ipLocationService.hydrateIpLocationRecords(activeRecords, (record) =>
      ipLocationRefs.whitelist(record.id),
    );
    return activeRecords;
  }

  private async findExactIPRecordsWithScan(
    ip: string,
    rebuildIndex: boolean,
  ): Promise<WhiteListRecord[]> {
    const normalizedIp = normalizeIp(ip) || String(ip || "").trim();
    const allRecords = await this.redis.hgetall(KEYS.RECORDS);
    const records: WhiteListRecord[] = [];
    const ids: string[] = [];

    for (const [id, raw] of Object.entries(allRecords)) {
      const record = deserializeRecord(raw);
      if (!record) continue;
      const matchesExactIp =
        isIPRecord(record) && normalizeIp(record.ip || "") === normalizedIp;
      const matchesResolvedCname =
        isCNAMERecord(record) &&
        getCnameResolvedTargets(record).includes(normalizedIp);
      if (
        (matchesExactIp || matchesResolvedCname) &&
        record.status === "active"
      ) {
        records.push(record);
        ids.push(id);
      }
    }

    sortRecordsByCreatedAtDesc(records);
    if (!rebuildIndex) return records;

    const ipKey = getIPRecordsKey(normalizedIp);
    const pipeline = this.redis.pipeline();
    pipeline.del(ipKey);
    if (ids.length > 0) {
      pipeline.sadd(ipKey, ...ids);
    }
    await pipeline.exec();
    return records;
  }

  private async findAllActiveCIDRRecordsWithScan(
    rebuildIndex: boolean,
  ): Promise<WhiteListRecord[]> {
    const allRecords = await this.redis.hgetall(KEYS.RECORDS);
    const records: WhiteListRecord[] = [];
    const ids: string[] = [];

    for (const [id, raw] of Object.entries(allRecords)) {
      const record = deserializeRecord(raw);
      if (!record) continue;
      if (isCIDRRecord(record) && record.status === "active") {
        records.push(record);
        ids.push(id);
      }
    }

    sortRecordsByCreatedAtDesc(records);
    if (!rebuildIndex) return records;

    const pipeline = this.redis.pipeline();
    pipeline.del(KEYS.CIDR_RECORDS);
    if (ids.length > 0) {
      pipeline.sadd(KEYS.CIDR_RECORDS, ...ids);
    }
    await pipeline.exec();
    return records;
  }
}
