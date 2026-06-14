import type Redis from "ioredis";
import { createHash } from "node:crypto";
import { resolve4, resolve6 } from "node:dns/promises";
import { v4 as uuidv4 } from "uuid";
import { goBackend } from "./go-backend";
import { configManager, redis } from "./redis";
import { ipLocationRefs, ipLocationService } from "./ip-location";
import { normalizeIp } from "./ip-normalize";
import { shouldAutoManageFirewallForRunType } from "./firewall-automation";
import {
  doesClientIpMatchWhiteListTarget,
  inferWhiteListTargetType,
  normalizeWhiteListTarget,
  type WhiteListTargetType,
} from "./whitelist-target";
import { tDefault } from "./i18n";

export interface WhiteListRecord {
  id: string;
  ip: string;
  targetType: WhiteListTargetType;
  expireAt: number | null;
  source: "manual" | "auto";
  createdAt: number;
  comment?: string;
  status: "active" | "expired" | "deleted";
  ipLocation?: string;
  resolvedTargets?: string[];
  checkIntervalMinutes?: number | null;
  lastCheckedAt?: number | null;
  lastResolvedAt?: number | null;
  resolveStatus?: "pending" | "resolved" | "empty" | "error";
  resolveMessage?: string;
}

export interface WhiteListConcreteTargetRecord {
  recordId: string;
  recordTarget: string;
  recordTargetType: WhiteListTargetType;
  source: WhiteListRecord["source"];
  target: string;
  targetType: "ip" | "cidr";
}

type WhiteListAddInput = Pick<WhiteListRecord, "ip" | "expireAt" | "source"> &
  Partial<
    Pick<WhiteListRecord, "comment" | "targetType" | "checkIntervalMinutes">
  >;

type SessionAutoWhiteListInput = {
  ownerKey: string;
  ip: string;
  expireAt: number | null;
  comment?: string;
  existingRecordId?: string | null;
};

type CnameRefreshResult = {
  record: WhiteListRecord;
  changed: boolean;
  skipped: boolean;
  syncError?: string;
};

const PREFIX = "fn_knock:whitelist";
const DEFAULT_CNAME_CHECK_INTERVAL_MINUTES = 5;
const MIN_CNAME_CHECK_INTERVAL_MINUTES = 1;
const MAX_CNAME_CHECK_INTERVAL_MINUTES = 24 * 60;
const KEYS = {
  RECORDS: `${PREFIX}:records`,
  RECORD_ORDER: `${PREFIX}:record_order`,
  EXPIRY: `${PREFIX}:expiry`,
  IPS: `${PREFIX}:ips`,
  CIDR_RECORDS: `${PREFIX}:cidr_records`,
  DELETED: `${PREFIX}:deleted`,
};
const whitelistManagerT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.whitelistManager.${key}`, params);

const getRecordTargetType = (
  record: Partial<Pick<WhiteListRecord, "targetType">>,
): WhiteListTargetType =>
  record.targetType === "cidr"
    ? "cidr"
    : record.targetType === "cname"
      ? "cname"
      : "ip";

const getRecordTarget = (
  record: Partial<Pick<WhiteListRecord, "ip">>,
): string => String(record.ip || "").trim();

const isIPRecord = (
  record: Partial<Pick<WhiteListRecord, "targetType">>,
): boolean => getRecordTargetType(record) === "ip";

const isCIDRRecord = (
  record: Partial<Pick<WhiteListRecord, "targetType">>,
): boolean => getRecordTargetType(record) === "cidr";

const isCNAMERecord = (
  record: Partial<Pick<WhiteListRecord, "targetType">>,
): boolean => getRecordTargetType(record) === "cname";

const normalizeCnameCheckIntervalMinutes = (value: unknown): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed)) {
    return DEFAULT_CNAME_CHECK_INTERVAL_MINUTES;
  }

  return Math.min(
    MAX_CNAME_CHECK_INTERVAL_MINUTES,
    Math.max(MIN_CNAME_CHECK_INTERVAL_MINUTES, Math.floor(parsed)),
  );
};

const normalizeResolvedTargets = (value: unknown): string[] => {
  if (!Array.isArray(value)) return [];

  const targets = new Set<string>();
  for (const item of value) {
    const normalized = normalizeIp(String(item ?? "").trim());
    if (!normalized) continue;
    targets.add(normalized);
  }

  return [...targets].sort((left, right) => left.localeCompare(right));
};

const toOptionalTimestamp = (value: unknown): number | null => {
  if (value === null || value === undefined || value === "") {
    return null;
  }

  const parsed = Number.parseInt(String(value), 10);
  return Number.isFinite(parsed) ? parsed : null;
};

const getCnameResolvedTargets = (
  record: Partial<Pick<WhiteListRecord, "resolvedTargets" | "targetType">>,
): string[] =>
  isCNAMERecord(record) ? normalizeResolvedTargets(record.resolvedTargets) : [];

const getConcreteIPTargets = (
  record: Partial<
    Pick<WhiteListRecord, "ip" | "resolvedTargets" | "targetType">
  >,
): string[] => {
  if (isIPRecord(record)) {
    const normalized = normalizeIp(getRecordTarget(record));
    return normalized ? [normalized] : [];
  }

  if (isCNAMERecord(record)) {
    return getCnameResolvedTargets(record);
  }

  return [];
};

const getConcreteTargets = (
  record: Partial<
    Pick<WhiteListRecord, "ip" | "resolvedTargets" | "targetType">
  >,
): Array<{ target: string; targetType: "ip" | "cidr" }> => {
  if (isCIDRRecord(record)) {
    const target = getRecordTarget(record);
    return target ? [{ target, targetType: "cidr" }] : [];
  }

  return getConcreteIPTargets(record).map((target) => ({
    target,
    targetType: "ip" as const,
  }));
};

const sortRecordsByCreatedAtDesc = (
  records: WhiteListRecord[],
): WhiteListRecord[] =>
  records.sort((left, right) => right.createdAt - left.createdAt);

const deserializeRecord = (raw: string): WhiteListRecord | null => {
  try {
    const parsed = JSON.parse(raw) as Partial<WhiteListRecord>;
    const id = String(parsed.id || "").trim();
    if (!id) return null;

    const rawTarget = getRecordTarget(parsed);
    const targetType =
      parsed.targetType === "cidr"
        ? "cidr"
        : parsed.targetType === "cname"
          ? "cname"
          : (inferWhiteListTargetType(rawTarget) ?? "ip");
    const normalizedTarget = normalizeWhiteListTarget(rawTarget, targetType);
    if (!normalizedTarget) return null;

    const source = parsed.source === "auto" ? "auto" : "manual";
    const status =
      parsed.status === "expired" || parsed.status === "deleted"
        ? parsed.status
        : "active";
    const createdAt = Number.parseInt(String(parsed.createdAt ?? 0), 10);
    const expireAt = toOptionalTimestamp(parsed.expireAt);
    const comment =
      typeof parsed.comment === "string" ? parsed.comment : undefined;
    const ipLocation =
      targetType === "ip" && typeof parsed.ipLocation === "string"
        ? parsed.ipLocation
        : undefined;
    const resolvedTargets =
      targetType === "cname"
        ? normalizeResolvedTargets(parsed.resolvedTargets)
        : undefined;
    const checkIntervalMinutes =
      targetType === "cname"
        ? normalizeCnameCheckIntervalMinutes(parsed.checkIntervalMinutes)
        : null;
    const lastCheckedAt = toOptionalTimestamp(parsed.lastCheckedAt);
    const lastResolvedAt = toOptionalTimestamp(parsed.lastResolvedAt);
    const resolveStatus =
      parsed.resolveStatus === "resolved" ||
      parsed.resolveStatus === "empty" ||
      parsed.resolveStatus === "error" ||
      parsed.resolveStatus === "pending"
        ? parsed.resolveStatus
        : targetType === "cname"
          ? "pending"
          : undefined;
    const resolveMessage =
      typeof parsed.resolveMessage === "string"
        ? parsed.resolveMessage.trim() || undefined
        : undefined;

    return {
      id,
      ip: normalizedTarget,
      targetType,
      expireAt:
        expireAt !== null && Number.isFinite(expireAt) ? expireAt : null,
      source,
      createdAt: Number.isFinite(createdAt) ? createdAt : 0,
      ...(comment !== undefined ? { comment } : {}),
      status,
      ...(ipLocation ? { ipLocation } : {}),
      ...(resolvedTargets !== undefined ? { resolvedTargets } : {}),
      ...(checkIntervalMinutes !== null ? { checkIntervalMinutes } : {}),
      ...(lastCheckedAt !== null ? { lastCheckedAt } : {}),
      ...(lastResolvedAt !== null ? { lastResolvedAt } : {}),
      ...(resolveStatus ? { resolveStatus } : {}),
      ...(resolveMessage ? { resolveMessage } : {}),
    };
  } catch {
    return null;
  }
};

export class IPTablesWhiteListManager {
  private redis: Redis;
  private cnameRefreshTasks = new Map<
    string,
    Promise<CnameRefreshResult | null>
  >();

  constructor() {
    this.redis = redis;
  }

  private getIPRecordsKey(ip: string) {
    const normalizedIp = normalizeIp(ip) || String(ip || "").trim();
    return `${PREFIX}:ip_records:${normalizedIp}`;
  }

  private getAutoOwnerRecordKey(ownerKey: string) {
    const digest = createHash("sha256")
      .update(String(ownerKey || "").trim())
      .digest("hex");
    return `${PREFIX}:auto_owner:${digest}`;
  }

  private getNow(): number {
    return Math.floor(Date.now() / 1000);
  }

  private buildConcreteTargetRecords(
    record: WhiteListRecord,
  ): WhiteListConcreteTargetRecord[] {
    return getConcreteTargets(record).map((entry) => ({
      recordId: record.id,
      recordTarget: record.ip,
      recordTargetType: record.targetType,
      source: record.source,
      target: entry.target,
      targetType: entry.targetType,
    }));
  }

  private isNoDataResolveError(error: unknown): boolean {
    const code = String((error as any)?.code || "").toUpperCase();
    return (
      code === "ENODATA" ||
      code === "ENOTFOUND" ||
      code === "EAI_NODATA" ||
      code === "EAI_NONAME"
    );
  }

  private formatResolveError(label: "A" | "AAAA", error: unknown): string {
    const code = String((error as any)?.code || "").trim();
    const message = (error as any)?.message || String(error);
    return code
      ? whitelistManagerT("dnsRecordQueryFailedWithCode", {
          label,
          code,
          message,
        })
      : whitelistManagerT("dnsRecordQueryFailed", { label, message });
  }

  private async resolveCnameTargets(domain: string): Promise<string[]> {
    const [ipv4Result, ipv6Result] = await Promise.allSettled([
      resolve4(domain),
      resolve6(domain),
    ]);
    const hardErrors: string[] = [];
    const resolvedTargets = new Set<string>();

    if (ipv4Result.status === "fulfilled") {
      for (const ip of ipv4Result.value) {
        const normalized = normalizeIp(ip);
        if (normalized) resolvedTargets.add(normalized);
      }
    } else if (!this.isNoDataResolveError(ipv4Result.reason)) {
      hardErrors.push(this.formatResolveError("A", ipv4Result.reason));
    }

    if (ipv6Result.status === "fulfilled") {
      for (const ip of ipv6Result.value) {
        const normalized = normalizeIp(ip);
        if (normalized) resolvedTargets.add(normalized);
      }
    } else if (!this.isNoDataResolveError(ipv6Result.reason)) {
      hardErrors.push(this.formatResolveError("AAAA", ipv6Result.reason));
    }

    if (hardErrors.length > 0) {
      throw new Error(hardErrors.join("；"));
    }

    return [...resolvedTargets].sort((left, right) =>
      left.localeCompare(right),
    );
  }

  private isCnameRefreshDue(
    record: WhiteListRecord,
    now = this.getNow(),
  ): boolean {
    if (!isCNAMERecord(record) || record.status !== "active") {
      return false;
    }

    if (record.expireAt && record.expireAt <= now) {
      return false;
    }

    const intervalMinutes = normalizeCnameCheckIntervalMinutes(
      record.checkIntervalMinutes,
    );
    const lastCheckedAt = toOptionalTimestamp(record.lastCheckedAt);
    if (lastCheckedAt === null) {
      return true;
    }

    return lastCheckedAt + intervalMinutes * 60 <= now;
  }

  private async cleanupUnusedConcreteTargets(
    targets: Array<{ target: string; targetType: "ip" | "cidr" }>,
  ): Promise<void> {
    const uniqueTargets = new Map<string, "ip" | "cidr">();
    for (const entry of targets) {
      if (!entry.target) continue;
      uniqueTargets.set(
        `${entry.targetType}:${entry.target}`,
        entry.targetType,
      );
    }

    for (const [key, targetType] of uniqueTargets.entries()) {
      const target = key.slice(targetType.length + 1);
      if (targetType === "cidr") {
        const active = await this.findRecordsByTarget(target, "cidr");
        if (active.length > 0) continue;
        await this.removeAllowedTarget(target);
        continue;
      }

      const active = await this.findExactIPRecords(target);
      if (active.length > 0) continue;
      await this.redis.srem(KEYS.IPS, target);
      await this.redis.del(this.getIPRecordsKey(target));
      await this.removeAllowedTarget(target);
    }
  }

  private async shouldSyncDirectModeFirewall(): Promise<boolean> {
    const config = await configManager.getConfig();
    return shouldAutoManageFirewallForRunType(config.run_type, config);
  }

  private async syncAllowedTarget(target: string) {
    if (!(await this.shouldSyncDirectModeFirewall())) return;
    await goBackend.allowIP(target);
  }

  private async removeAllowedTarget(target: string) {
    if (!(await this.shouldSyncDirectModeFirewall())) return;
    await goBackend.removeIP(target);
  }

  private normalizeTargetInput(
    value: string,
    source: WhiteListRecord["source"],
    targetType?: WhiteListTargetType,
  ): { target: string; targetType: WhiteListTargetType } {
    const inferredType = targetType ?? inferWhiteListTargetType(value);
    if (!inferredType) {
      throw new Error(whitelistManagerT("targetFormatInvalid"));
    }
    if (source === "auto" && inferredType !== "ip") {
      throw new Error(whitelistManagerT("autoGrantIpOnly"));
    }

    const target = normalizeWhiteListTarget(value, inferredType);
    if (!target) {
      throw new Error(
        inferredType === "cidr"
          ? whitelistManagerT("cidrInvalid")
          : inferredType === "cname"
            ? whitelistManagerT("domainInvalid")
            : whitelistManagerT("ipInvalid"),
      );
    }

    return {
      target,
      targetType: inferredType,
    };
  }

  async getRecordById(id: string): Promise<WhiteListRecord | null> {
    const raw = await this.redis.hget(KEYS.RECORDS, id);
    if (!raw) return null;
    return deserializeRecord(raw);
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

    const ipKey = this.getIPRecordsKey(normalizedIp);
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

  private async getAllActiveCIDRRecords(): Promise<WhiteListRecord[]> {
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

  private async rebuildRecordOrderIndex(): Promise<WhiteListRecord[]> {
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
      pipeline.del(this.getIPRecordsKey(ip));
    }

    for (const record of activeRecords) {
      pipeline.zadd(KEYS.RECORD_ORDER, record.createdAt, record.id);
      if (record.expireAt) {
        pipeline.zadd(KEYS.EXPIRY, record.expireAt, record.id);
      }
    }

    for (const [ip, ids] of ipRecordIds.entries()) {
      pipeline.sadd(KEYS.IPS, ip);
      pipeline.sadd(this.getIPRecordsKey(ip), ...ids);
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

  async addWhiteList(
    record: WhiteListAddInput,
    options?: { replaceSource?: "manual" | "auto" | "all" },
  ): Promise<string> {
    const { target, targetType } = this.normalizeTargetInput(
      record.ip,
      record.source,
      record.targetType,
    );
    const replaceSource = options?.replaceSource ?? record.source;
    if (replaceSource === "all") {
      await this.removeRecordsByTarget(target, targetType);
    } else {
      await this.removeRecordsByTarget(target, targetType, replaceSource);
    }

    const id = `whitelist:${uuidv4()}`;
    const now = this.getNow();
    const ipLocationStr =
      targetType === "ip"
        ? await ipLocationService.getCachedLocation(target)
        : "";
    const fullRecord: WhiteListRecord = {
      ip: target,
      expireAt: record.expireAt,
      source: record.source,
      targetType,
      id,
      createdAt: now,
      status: "active",
      ...(record.comment !== undefined ? { comment: record.comment } : {}),
      ...(ipLocationStr ? { ipLocation: ipLocationStr } : {}),
      ...(targetType === "cname"
        ? {
            resolvedTargets: [] as string[],
            checkIntervalMinutes: normalizeCnameCheckIntervalMinutes(
              record.checkIntervalMinutes,
            ),
            resolveStatus: "pending" as const,
          }
        : {}),
    };

    const pipeline = this.redis.pipeline();
    pipeline.hset(KEYS.RECORDS, id, JSON.stringify(fullRecord));
    pipeline.zadd(KEYS.RECORD_ORDER, now, id);

    if (targetType === "ip") {
      const ipKey = this.getIPRecordsKey(target);
      pipeline.sadd(KEYS.IPS, target);
      pipeline.sadd(ipKey, id);
    } else if (targetType === "cidr") {
      pipeline.sadd(KEYS.CIDR_RECORDS, id);
    }

    if (record.expireAt) {
      pipeline.zadd(KEYS.EXPIRY, record.expireAt, id);
    }

    await pipeline.exec();
    if (targetType === "ip") {
      await ipLocationService.registerUsage(target, [
        ipLocationRefs.whitelist(id),
      ]);
    }
    if (targetType === "cname") {
      await this.refreshCnameRecord(id, { force: true });
      return id;
    }

    await this.syncAllowedTarget(target);
    return id;
  }

  async ensureSessionAutoWhiteList(
    input: SessionAutoWhiteListInput,
  ): Promise<WhiteListRecord> {
    const ownerKey = String(input.ownerKey || "").trim();
    if (!ownerKey) {
      throw new Error(whitelistManagerT("autoOwnerMissing"));
    }

    const { target, targetType } = this.normalizeTargetInput(
      input.ip,
      "auto",
      "ip",
    );
    const ownerRecordKey = this.getAutoOwnerRecordKey(ownerKey);
    const ownerRecordId = await this.redis.get(ownerRecordKey);
    const candidateIds = [
      input.existingRecordId || "",
      ownerRecordId || "",
    ].filter((id, index, all) => id && all.indexOf(id) === index);

    for (const candidateId of candidateIds) {
      const existing = await this.getRecordById(candidateId);
      if (!existing || existing.source !== "auto" || !isIPRecord(existing)) {
        continue;
      }

      const now = this.getNow();
      if (
        existing.status !== "active" ||
        (existing.expireAt && existing.expireAt <= now)
      ) {
        await this.removeWhiteList(candidateId);
        continue;
      }

      const updated = await this.updateSessionAutoWhiteListRecord(existing, {
        ip: target,
        expireAt: input.expireAt,
        ...(input.comment !== undefined ? { comment: input.comment } : {}),
      });
      await this.saveAutoOwnerRecordId(
        ownerRecordKey,
        updated.id,
        input.expireAt,
      );
      return updated;
    }

    const id = `whitelist:${uuidv4()}`;
    const now = this.getNow();
    const ipLocationStr = await ipLocationService.getCachedLocation(target);
    const fullRecord: WhiteListRecord = {
      ip: target,
      targetType,
      expireAt: input.expireAt,
      source: "auto",
      id,
      createdAt: now,
      status: "active",
      ...(input.comment !== undefined ? { comment: input.comment } : {}),
      ...(ipLocationStr ? { ipLocation: ipLocationStr } : {}),
    };

    const pipeline = this.redis.pipeline();
    pipeline.hset(KEYS.RECORDS, id, JSON.stringify(fullRecord));
    pipeline.zadd(KEYS.RECORD_ORDER, now, id);
    pipeline.sadd(KEYS.IPS, target);
    pipeline.sadd(this.getIPRecordsKey(target), id);
    if (input.expireAt) {
      pipeline.zadd(KEYS.EXPIRY, input.expireAt, id);
    }
    this.queueAutoOwnerRecordId(pipeline, ownerRecordKey, id, input.expireAt);
    await pipeline.exec();
    await ipLocationService.registerUsage(target, [
      ipLocationRefs.whitelist(id),
    ]);
    await this.syncAllowedTarget(target);
    return fullRecord;
  }

  private queueAutoOwnerRecordId(
    pipeline: ReturnType<Redis["pipeline"]>,
    ownerRecordKey: string,
    recordId: string,
    expireAt: number | null,
  ) {
    const ttl = expireAt ? expireAt - this.getNow() : 0;
    if (ttl > 0) {
      pipeline.set(ownerRecordKey, recordId, "EX", ttl);
      return;
    }
    pipeline.set(ownerRecordKey, recordId);
  }

  private async saveAutoOwnerRecordId(
    ownerRecordKey: string,
    recordId: string,
    expireAt: number | null,
  ): Promise<void> {
    const ttl = expireAt ? expireAt - this.getNow() : 0;
    if (ttl > 0) {
      await this.redis.set(ownerRecordKey, recordId, "EX", ttl);
      return;
    }
    await this.redis.set(ownerRecordKey, recordId);
  }

  private async updateSessionAutoWhiteListRecord(
    record: WhiteListRecord,
    updates: Pick<WhiteListRecord, "ip" | "expireAt"> &
      Partial<Pick<WhiteListRecord, "comment">>,
  ): Promise<WhiteListRecord> {
    const oldConcreteTargets = this.buildConcreteTargetRecords(record).map(
      (entry) => ({ target: entry.target, targetType: entry.targetType }),
    );
    const oldIp = normalizeIp(record.ip) || record.ip;
    const normalizedIp =
      normalizeIp(updates.ip) || String(updates.ip || "").trim();
    const ipLocationStr =
      await ipLocationService.getCachedLocation(normalizedIp);
    const nextRecord: WhiteListRecord = {
      ...record,
      ip: normalizedIp,
      targetType: "ip",
      expireAt: updates.expireAt,
      ...(updates.comment !== undefined ? { comment: updates.comment } : {}),
      ...(ipLocationStr ? { ipLocation: ipLocationStr } : {}),
    };
    const nextConcreteTargets = this.buildConcreteTargetRecords(nextRecord).map(
      (entry) => ({ target: entry.target, targetType: entry.targetType }),
    );

    const pipeline = this.redis.pipeline();
    pipeline.hset(KEYS.RECORDS, record.id, JSON.stringify(nextRecord));
    if (updates.expireAt) {
      pipeline.zadd(KEYS.EXPIRY, updates.expireAt, record.id);
    } else {
      pipeline.zrem(KEYS.EXPIRY, record.id);
    }
    if (oldIp !== normalizedIp) {
      pipeline.srem(this.getIPRecordsKey(oldIp), record.id);
      pipeline.sadd(KEYS.IPS, normalizedIp);
      pipeline.sadd(this.getIPRecordsKey(normalizedIp), record.id);
    }
    await pipeline.exec();
    await ipLocationService.registerUsage(normalizedIp, [
      ipLocationRefs.whitelist(record.id),
    ]);
    await this.syncAllowedTarget(normalizedIp);
    await this.cleanupUnusedConcreteTargets(
      oldConcreteTargets.filter(
        (oldTarget) =>
          !nextConcreteTargets.some(
            (nextTarget) =>
              nextTarget.targetType === oldTarget.targetType &&
              nextTarget.target === oldTarget.target,
          ),
      ),
    );
    return nextRecord;
  }

  async removeWhiteList(id: string): Promise<boolean> {
    const record = await this.getRecordById(id);
    if (!record) return false;

    const targetType = getRecordTargetType(record);
    const concreteTargets = getConcreteTargets(record);
    const pipeline = this.redis.pipeline();
    pipeline.hdel(KEYS.RECORDS, id);
    pipeline.hdel(KEYS.DELETED, id);
    pipeline.zrem(KEYS.RECORD_ORDER, id);
    pipeline.zrem(KEYS.EXPIRY, id);
    if (targetType === "cidr") {
      pipeline.srem(KEYS.CIDR_RECORDS, id);
    } else {
      for (const target of getConcreteIPTargets(record)) {
        pipeline.srem(this.getIPRecordsKey(target), id);
      }
    }
    await pipeline.exec();
    await this.cleanupUnusedConcreteTargets(concreteTargets);

    return true;
  }

  async updateComment(id: string, comment: string): Promise<boolean> {
    const record = await this.getRecordById(id);
    if (!record) return false;

    record.comment = comment;
    await this.redis.hset(KEYS.RECORDS, id, JSON.stringify(record));
    return true;
  }

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
        pipeline.srem(this.getIPRecordsKey(ip), ...staleIds);
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

  async getAllActiveConcreteTargets(
    source?: "manual" | "auto",
  ): Promise<WhiteListConcreteTargetRecord[]> {
    const now = this.getNow();
    const records = await this.getAllActiveRecords(source);
    const targets: WhiteListConcreteTargetRecord[] = [];

    for (const record of records) {
      if (record.expireAt && record.expireAt <= now) {
        continue;
      }

      targets.push(...this.buildConcreteTargetRecords(record));
    }

    return targets;
  }

  async isIPWhitelisted(ip: string): Promise<boolean> {
    return this.hasValidIP(ip);
  }

  async hasValidIP(ip: string): Promise<boolean> {
    const records = await this.getActiveRecordsByIP(ip);
    return records.length > 0;
  }

  private async findExactIPRecords(ip: string): Promise<WhiteListRecord[]> {
    const normalizedIp = normalizeIp(ip) || String(ip || "").trim();
    if (!normalizedIp) return [];

    const ipKey = this.getIPRecordsKey(normalizedIp);
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

  private async findMatchingCIDRRecords(
    ip: string,
  ): Promise<WhiteListRecord[]> {
    const normalizedIp = normalizeIp(ip) || String(ip || "").trim();
    if (!normalizedIp) return [];

    const records = await this.getAllActiveCIDRRecords();
    const now = Math.floor(Date.now() / 1000);
    return sortRecordsByCreatedAtDesc(
      records.filter((record) => {
        if (record.expireAt && record.expireAt <= now) return false;
        return doesClientIpMatchWhiteListTarget(
          normalizedIp,
          record.ip,
          record.targetType,
        );
      }),
    );
  }

  private async findRecordsByTarget(
    target: string,
    targetType: WhiteListTargetType,
  ): Promise<WhiteListRecord[]> {
    if (targetType === "cidr") {
      const records = await this.getAllActiveCIDRRecords();
      return sortRecordsByCreatedAtDesc(
        records.filter((record) => record.ip === target),
      );
    }

    if (targetType === "cname") {
      const records = await this.getAllActiveRecords();
      return sortRecordsByCreatedAtDesc(
        records.filter(
          (record) => isCNAMERecord(record) && record.ip === target,
        ),
      );
    }

    const normalizedIp = normalizeIp(target) || String(target || "").trim();
    if (!normalizedIp) return [];

    const records = await this.findExactIPRecords(normalizedIp);
    return sortRecordsByCreatedAtDesc(
      records.filter(
        (record) =>
          isIPRecord(record) &&
          (normalizeIp(record.ip || "") === normalizedIp ||
            record.ip === normalizedIp),
      ),
    );
  }

  async getActiveRecordsByIP(
    ip: string,
    source?: "manual" | "auto",
  ): Promise<WhiteListRecord[]> {
    const [exactRecords, cidrRecords] = await Promise.all([
      this.findExactIPRecords(ip),
      this.findMatchingCIDRRecords(ip),
    ]);
    const now = Math.floor(Date.now() / 1000);

    return sortRecordsByCreatedAtDesc(
      [...exactRecords, ...cidrRecords].filter((record) => {
        if (record.status !== "active") return false;
        if (record.expireAt && record.expireAt <= now) return false;
        if (source && record.source !== source) return false;
        return true;
      }),
    );
  }

  async getLatestActiveRecordByIP(
    ip: string,
    source?: "manual" | "auto",
  ): Promise<WhiteListRecord | null> {
    const records = await this.getActiveRecordsByIP(ip, source);
    return records[0] || null;
  }

  async moveRecordToIP(
    id: string,
    newIp: string,
  ): Promise<WhiteListRecord | null> {
    const record = await this.getRecordById(id);
    if (!record || record.status !== "active" || !isIPRecord(record)) {
      return null;
    }

    const now = Math.floor(Date.now() / 1000);
    if (record.expireAt && record.expireAt <= now) return null;

    const oldIp = normalizeIp(record.ip) || record.ip;
    const normalizedNewIp = normalizeIp(newIp) || String(newIp || "").trim();
    if (!normalizedNewIp) return null;
    if (oldIp === normalizedNewIp) {
      return record;
    }

    const ipLocationStr =
      await ipLocationService.getCachedLocation(normalizedNewIp);
    const nextRecord: WhiteListRecord = {
      ...record,
      ip: normalizedNewIp,
      targetType: "ip",
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
    await ipLocationService.registerUsage(normalizedNewIp, [
      ipLocationRefs.whitelist(id),
    ]);

    await this.syncAllowedTarget(normalizedNewIp);

    const remainingOldRecords = await this.findExactIPRecords(oldIp);
    if (remainingOldRecords.length === 0) {
      await this.redis.srem(KEYS.IPS, oldIp);
      await this.redis.del(oldIpKey);
      await this.removeAllowedTarget(oldIp);
    }

    return nextRecord;
  }

  async refreshCnameRecord(
    id: string,
    options: { force?: boolean } = {},
  ): Promise<CnameRefreshResult | null> {
    const existingTask = this.cnameRefreshTasks.get(id);
    if (existingTask) {
      return existingTask;
    }

    const task = (async () => {
      const record = await this.getRecordById(id);
      if (!record || !isCNAMERecord(record) || record.status !== "active") {
        return null;
      }

      const now = this.getNow();
      if (!options.force && !this.isCnameRefreshDue(record, now)) {
        return {
          record,
          changed: false,
          skipped: true,
        };
      }

      const previousTargets = getConcreteIPTargets(record);
      let resolvedTargets: string[];

      try {
        resolvedTargets = await this.resolveCnameTargets(record.ip);
      } catch (error: any) {
        const nextRecord: WhiteListRecord = {
          ...record,
          resolvedTargets: [],
          checkIntervalMinutes: normalizeCnameCheckIntervalMinutes(
            record.checkIntervalMinutes,
          ),
          lastCheckedAt: now,
          resolveStatus: "error",
          resolveMessage: error?.message || whitelistManagerT("domainResolveFailed"),
        };
        const pipeline = this.redis.pipeline();
        pipeline.hset(KEYS.RECORDS, id, JSON.stringify(nextRecord));
        for (const target of previousTargets) {
          pipeline.srem(this.getIPRecordsKey(target), id);
        }
        await pipeline.exec();
        await this.cleanupUnusedConcreteTargets(
          previousTargets.map((target) => ({
            target,
            targetType: "ip" as const,
          })),
        );

        return {
          record: nextRecord,
          changed: previousTargets.length > 0,
          skipped: false,
        };
      }

      const changed =
        resolvedTargets.length !== previousTargets.length ||
        resolvedTargets.some(
          (target, index) => target !== previousTargets[index],
        );
      const nextRecord: WhiteListRecord = {
        ...record,
        resolvedTargets,
        checkIntervalMinutes: normalizeCnameCheckIntervalMinutes(
          record.checkIntervalMinutes,
        ),
        lastCheckedAt: now,
        lastResolvedAt: now,
        resolveStatus: resolvedTargets.length > 0 ? "resolved" : "empty",
        resolveMessage:
          resolvedTargets.length > 0
            ? whitelistManagerT("resolvedIpCount", {
                count: resolvedTargets.length,
              })
            : whitelistManagerT("noAaaaRecords"),
      };
      const nextTargets = getConcreteIPTargets(nextRecord);
      const previousTargetSet = new Set(previousTargets);
      const nextTargetSet = new Set(nextTargets);
      const addedTargets = nextTargets.filter(
        (target) => !previousTargetSet.has(target),
      );
      const removedTargets = previousTargets.filter(
        (target) => !nextTargetSet.has(target),
      );

      const pipeline = this.redis.pipeline();
      pipeline.hset(KEYS.RECORDS, id, JSON.stringify(nextRecord));
      if (addedTargets.length > 0) {
        pipeline.sadd(KEYS.IPS, ...addedTargets);
        for (const target of addedTargets) {
          pipeline.sadd(this.getIPRecordsKey(target), id);
        }
      }
      for (const target of removedTargets) {
        pipeline.srem(this.getIPRecordsKey(target), id);
      }
      await pipeline.exec();

      let syncError: string | undefined;
      try {
        for (const target of addedTargets) {
          await this.syncAllowedTarget(target);
        }
        await this.cleanupUnusedConcreteTargets(
          removedTargets.map((target) => ({
            target,
            targetType: "ip" as const,
          })),
        );
      } catch (error: any) {
        syncError =
          error?.message || whitelistManagerT("syncAllowedStateFailed");
        console.error(
          `[whitelist] failed to sync concrete targets for ${record.ip}:`,
          error,
        );
      }

      return {
        record: nextRecord,
        changed,
        skipped: false,
        ...(syncError ? { syncError } : {}),
      };
    })();

    this.cnameRefreshTasks.set(id, task);
    try {
      return await task;
    } finally {
      this.cnameRefreshTasks.delete(id);
    }
  }

  async processDueCnameRecords(): Promise<boolean> {
    const now = this.getNow();
    const records = await this.getAllActiveRecords("manual");
    let changed = false;

    for (const record of records) {
      if (!isCNAMERecord(record) || !this.isCnameRefreshDue(record, now)) {
        continue;
      }

      const result = await this.refreshCnameRecord(record.id);
      if (result?.changed) {
        changed = true;
      }
    }

    return changed;
  }

  private async removeRecordsByTarget(
    target: string,
    targetType: WhiteListTargetType,
    source?: "manual" | "auto",
  ): Promise<boolean> {
    const records = await this.findRecordsByTarget(target, targetType);
    let removed = false;
    for (const record of records) {
      if (!source || record.source === source) {
        const result = await this.removeWhiteList(record.id);
        if (result) removed = true;
      }
    }
    return removed;
  }

  async removeRecordsByIP(
    ip: string,
    source?: "manual" | "auto",
  ): Promise<boolean> {
    const normalizedIp = normalizeIp(ip) || String(ip || "").trim();
    if (!normalizedIp) return false;
    return this.removeRecordsByTarget(normalizedIp, "ip", source);
  }

  async removeRecordsBySource(source: "manual" | "auto"): Promise<number> {
    const records = await this.getAllActiveRecords(source);
    let removedCount = 0;

    for (const record of records) {
      if (await this.removeWhiteList(record.id)) {
        removedCount += 1;
      }
    }

    return removedCount;
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

      const record = deserializeRecord(raw);
      if (!record) {
        staleIds.push(id);
        return;
      }
      if (record.status !== "active") {
        staleIds.push(id);
        return;
      }

      records.push(record);
    });

    if (staleIds.length > 0) {
      await this.redis.zrem(KEYS.EXPIRY, ...staleIds);
    }
    return records;
  }

  async processExpiredRecords(): Promise<boolean> {
    try {
      const expiredRecords = await this.findExpiredRecords();
      if (expiredRecords.length === 0) return false;

      const touchedTargets: Array<{
        target: string;
        targetType: "ip" | "cidr";
      }> = [];
      const pipeline = this.redis.pipeline();

      for (const record of expiredRecords) {
        record.status = "expired";
        if (isCIDRRecord(record)) {
          pipeline.srem(KEYS.CIDR_RECORDS, record.id);
        } else {
          for (const target of getConcreteIPTargets(record)) {
            pipeline.srem(this.getIPRecordsKey(target), record.id);
          }
        }
        touchedTargets.push(...getConcreteTargets(record));
        pipeline.hset(KEYS.RECORDS, record.id, JSON.stringify(record));
        pipeline.zrem(KEYS.EXPIRY, record.id);
        pipeline.zrem(KEYS.RECORD_ORDER, record.id);
      }

      await pipeline.exec();
      await this.cleanupUnusedConcreteTargets(touchedTargets);

      return true;
    } catch (error) {
      console.error("Error processing expired records:", error);
      return false;
    }
  }

  async cleanup(): Promise<void> {
    return;
  }
}

export const whitelistManager = new IPTablesWhiteListManager();
