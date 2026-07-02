import { v4 as uuidv4 } from "uuid";
import { doesClientIpMatchWhiteListTarget } from "./whitelist-target";
import { redis } from "./redis";
import {
  resolveWhitelistRegionCidrs,
  type WhitelistRegionInput,
  type WhitelistRegionLookup,
} from "./whitelist-regions";
import type { WhiteListConcreteTargetRecord } from "./whitelist/record";

const PREFIX = "fn_knock:whitelist:region_groups";

const KEYS = {
  RECORDS: `${PREFIX}:records`,
  ORDER: `${PREFIX}:order`,
  EXPIRY: `${PREFIX}:expiry`,
};

export type WhitelistRegionGroupRecord = {
  id: string;
  regions: WhitelistRegionInput[];
  cidrs: string[];
  expireAt: number | null;
  source: "manual";
  createdAt: number;
  updatedAt: number;
  status: "active" | "deleted" | "expired";
  comment?: string;
};

export type WhitelistRegionGroupSummary = Omit<
  WhitelistRegionGroupRecord,
  "cidrs"
> & {
  cidrCount: number;
};

export type WhitelistRegionGroupCreateInput = {
  regions: unknown;
  expireAt: number | null;
  comment?: string;
  lookupCidrs: WhitelistRegionLookup;
};

const normalizeString = (value: unknown): string => String(value ?? "").trim();

const deserializeRegionGroup = (
  raw: string | null | undefined,
): WhitelistRegionGroupRecord | null => {
  if (!raw) return null;

  try {
    const parsed = JSON.parse(raw) as Partial<WhitelistRegionGroupRecord>;
    const id = normalizeString(parsed.id);
    if (!id) return null;

    const regions = Array.isArray(parsed.regions)
      ? parsed.regions
          .map((region) => ({
            province: normalizeString(region?.province),
            query_city: normalizeString(region?.query_city) || null,
          }))
          .filter((region) => region.province)
      : [];
    const cidrs = Array.isArray(parsed.cidrs)
      ? parsed.cidrs.map(normalizeString).filter(Boolean)
      : [];
    const createdAt = Number(parsed.createdAt);
    const updatedAt = Number(parsed.updatedAt);
    const expireAt =
      parsed.expireAt === null || parsed.expireAt === undefined
        ? null
        : Number(parsed.expireAt);
    const status =
      parsed.status === "deleted" || parsed.status === "expired"
        ? parsed.status
        : "active";

    return {
      id,
      regions,
      cidrs,
      expireAt: Number.isFinite(expireAt) ? expireAt : null,
      source: "manual",
      createdAt: Number.isFinite(createdAt) ? createdAt : 0,
      updatedAt: Number.isFinite(updatedAt) ? updatedAt : 0,
      status,
      ...(parsed.comment !== undefined
        ? { comment: normalizeString(parsed.comment) }
        : {}),
    };
  } catch {
    return null;
  }
};

const sortGroupsByCreatedAtDesc = (
  records: WhitelistRegionGroupRecord[],
): WhitelistRegionGroupRecord[] =>
  [...records].sort((left, right) => {
    if (right.createdAt !== left.createdAt) {
      return right.createdAt - left.createdAt;
    }
    return right.id.localeCompare(left.id);
  });

export const summarizeWhitelistRegionGroup = (
  record: WhitelistRegionGroupRecord,
): WhitelistRegionGroupSummary => {
  const { cidrs: _cidrs, ...rest } = record;
  return {
    ...rest,
    cidrCount: record.cidrs.length,
  };
};

export class WhitelistRegionGroupManager {
  private getNow(): number {
    return Math.floor(Date.now() / 1000);
  }

  private isUsableGroup(
    record: WhitelistRegionGroupRecord,
    now = this.getNow(),
  ): boolean {
    if (record.status !== "active") return false;
    if (record.expireAt && record.expireAt <= now) return false;
    return true;
  }

  async createGroup(
    input: WhitelistRegionGroupCreateInput,
  ): Promise<WhitelistRegionGroupRecord> {
    const resolved = await resolveWhitelistRegionCidrs({
      regions: input.regions,
      lookupCidrs: input.lookupCidrs,
    });
    const now = this.getNow();
    const id = `whitelist-region:${uuidv4()}`;
    const comment = normalizeString(input.comment);
    const record: WhitelistRegionGroupRecord = {
      id,
      regions: resolved.regions,
      cidrs: resolved.cidrs,
      expireAt: input.expireAt,
      source: "manual",
      createdAt: now,
      updatedAt: now,
      status: "active",
      ...(comment ? { comment } : {}),
    };

    const pipeline = redis.pipeline();
    pipeline.hset(KEYS.RECORDS, id, JSON.stringify(record));
    pipeline.zadd(KEYS.ORDER, now, id);
    if (record.expireAt) {
      pipeline.zadd(KEYS.EXPIRY, record.expireAt, id);
    }
    await pipeline.exec();

    return record;
  }

  async getRecordById(id: string): Promise<WhitelistRegionGroupRecord | null> {
    return deserializeRegionGroup(await redis.hget(KEYS.RECORDS, id));
  }

  async listActiveGroups(): Promise<WhitelistRegionGroupRecord[]> {
    const ids = await redis.zrevrange(KEYS.ORDER, 0, -1);
    if (ids.length === 0) {
      const allRecords = await redis.hgetall(KEYS.RECORDS);
      const records: WhitelistRegionGroupRecord[] = [];
      for (const raw of Object.values(allRecords)) {
        const record = deserializeRegionGroup(raw);
        if (record?.status === "active") {
          records.push(record);
        }
      }
      return sortGroupsByCreatedAtDesc(records);
    }

    const raws = await redis.hmget(KEYS.RECORDS, ...ids);
    const records: WhitelistRegionGroupRecord[] = [];
    const staleIds: string[] = [];

    raws.forEach((raw, index) => {
      const id = ids[index];
      if (!id) return;
      const record = deserializeRegionGroup(raw);
      if (!record || record.status !== "active") {
        staleIds.push(id);
        return;
      }
      records.push(record);
    });

    if (staleIds.length > 0) {
      const pipeline = redis.pipeline();
      pipeline.zrem(KEYS.ORDER, ...staleIds);
      pipeline.zrem(KEYS.EXPIRY, ...staleIds);
      await pipeline.exec();
    }

    return sortGroupsByCreatedAtDesc(records);
  }

  async listActiveGroupSummaries(): Promise<WhitelistRegionGroupSummary[]> {
    const groups = await this.listActiveGroups();
    return groups.map(summarizeWhitelistRegionGroup);
  }

  async deleteGroup(id: string): Promise<WhitelistRegionGroupRecord | null> {
    const record = await this.getRecordById(id);
    if (!record || record.status !== "active") return null;

    const nextRecord: WhitelistRegionGroupRecord = {
      ...record,
      status: "deleted",
      updatedAt: this.getNow(),
    };
    const pipeline = redis.pipeline();
    pipeline.hset(KEYS.RECORDS, id, JSON.stringify(nextRecord));
    pipeline.zrem(KEYS.ORDER, id);
    pipeline.zrem(KEYS.EXPIRY, id);
    await pipeline.exec();
    return record;
  }

  async processExpiredGroups(): Promise<WhitelistRegionGroupRecord[]> {
    const now = this.getNow();
    const expiredIds = await redis.zrangebyscore(KEYS.EXPIRY, 0, now);
    if (expiredIds.length === 0) return [];

    const raws = await redis.hmget(KEYS.RECORDS, ...expiredIds);
    const expiredRecords: WhitelistRegionGroupRecord[] = [];
    const staleIds: string[] = [];
    const pipeline = redis.pipeline();

    raws.forEach((raw, index) => {
      const id = expiredIds[index];
      if (!id) return;

      const record = deserializeRegionGroup(raw);
      if (!record || record.status !== "active") {
        staleIds.push(id);
        return;
      }

      const nextRecord: WhitelistRegionGroupRecord = {
        ...record,
        status: "expired",
        updatedAt: now,
      };
      expiredRecords.push(record);
      pipeline.hset(KEYS.RECORDS, id, JSON.stringify(nextRecord));
      pipeline.zrem(KEYS.ORDER, id);
      pipeline.zrem(KEYS.EXPIRY, id);
    });

    if (staleIds.length > 0) {
      pipeline.zrem(KEYS.ORDER, ...staleIds);
      pipeline.zrem(KEYS.EXPIRY, ...staleIds);
    }

    if (expiredRecords.length > 0 || staleIds.length > 0) {
      await pipeline.exec();
    }
    return expiredRecords;
  }

  async getActiveConcreteTargets(): Promise<WhiteListConcreteTargetRecord[]> {
    const now = this.getNow();
    const groups = await this.listActiveGroups();
    const targets: WhiteListConcreteTargetRecord[] = [];

    for (const group of groups) {
      if (!this.isUsableGroup(group, now)) continue;
      for (const cidr of group.cidrs) {
        targets.push({
          recordId: group.id,
          recordTarget: group.id,
          recordTargetType: "cidr",
          source: "manual",
          target: cidr,
          targetType: "cidr",
        });
      }
    }

    return targets;
  }

  async hasValidIP(ip: string): Promise<boolean> {
    const normalizedIp = normalizeString(ip);
    if (!normalizedIp) return false;

    const targets = await this.getActiveConcreteTargets();
    return targets.some((target) =>
      doesClientIpMatchWhiteListTarget(
        normalizedIp,
        target.target,
        target.targetType,
      ),
    );
  }

  async getActiveCIDRTargetSet(): Promise<Set<string>> {
    const targets = await this.getActiveConcreteTargets();
    return new Set(targets.map((target) => target.target.toLowerCase()));
  }

  async hasActiveCIDRTarget(target: string): Promise<boolean> {
    const normalizedTarget = normalizeString(target).toLowerCase();
    if (!normalizedTarget) return false;

    const targets = await this.getActiveCIDRTargetSet();
    return targets.has(normalizedTarget);
  }
}

export const whitelistRegionGroupManager = new WhitelistRegionGroupManager();
