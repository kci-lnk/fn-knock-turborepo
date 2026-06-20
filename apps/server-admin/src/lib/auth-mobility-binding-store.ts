import type Redis from "ioredis";
import {
  authMobilityKeys,
  type MobilitySubjectType,
} from "./auth-mobility-keys";

type RedisPipeline = ReturnType<Redis["pipeline"]>;

export type MobilityBinding = {
  version: 1;
  subjectType: MobilitySubjectType;
  subjectHash: string;
  currentIp: string;
  whitelistRecordId?: string;
  expireAt: number | null;
  ownerSessionId?: string;
  createdAt: string;
  lastSeenAt: string;
};

export const buildMobilityBinding = (args: {
  subjectType: MobilitySubjectType;
  subjectKey: string;
  currentIp: string;
  whitelistRecordId?: string;
  expireAt: number | null;
  ownerSessionId?: string;
}): MobilityBinding => {
  const nowIso = new Date().toISOString();
  return {
    version: 1,
    subjectType: args.subjectType,
    subjectHash: authMobilityKeys.subjectHash(
      args.subjectType,
      args.subjectKey,
    ),
    currentIp: args.currentIp,
    whitelistRecordId: args.whitelistRecordId,
    expireAt: args.expireAt,
    ownerSessionId: args.ownerSessionId,
    createdAt: nowIso,
    lastSeenAt: nowIso,
  };
};

const parseMobilityBinding = (
  raw: string | null | undefined,
): MobilityBinding | null => {
  if (!raw) return null;

  try {
    return JSON.parse(raw) as MobilityBinding;
  } catch {
    return null;
  }
};

export class AuthMobilityBindingStore {
  constructor(private readonly redis: Redis) {}

  storageKey(subjectType: MobilitySubjectType, subjectKey: string): string {
    return authMobilityKeys.binding(subjectType, subjectKey);
  }

  async get(
    subjectType: MobilitySubjectType,
    subjectKey: string,
  ): Promise<MobilityBinding | null> {
    return this.getByStorageKey(this.storageKey(subjectType, subjectKey));
  }

  async getByStorageKey(storageKey: string): Promise<MobilityBinding | null> {
    return parseMobilityBinding(await this.redis.get(storageKey));
  }

  async listSessionBindingKeys(sessionId: string): Promise<string[]> {
    return this.redis.smembers(authMobilityKeys.sessionIndex(sessionId));
  }

  async saveWithTtl(
    storageKey: string,
    binding: MobilityBinding,
    ttlSeconds: number,
  ): Promise<void> {
    await this.redis.set(storageKey, JSON.stringify(binding), "EX", ttlSeconds);
  }

  async saveKeepTtl(
    storageKey: string,
    binding: MobilityBinding,
  ): Promise<void> {
    await this.redis.set(storageKey, JSON.stringify(binding), "KEEPTTL");
  }

  async addSessionBinding(
    sessionId: string,
    storageKey: string,
  ): Promise<void> {
    await this.redis.sadd(authMobilityKeys.sessionIndex(sessionId), storageKey);
  }

  async removeSessionBinding(
    sessionId: string,
    storageKey: string,
  ): Promise<void> {
    await this.redis.srem(authMobilityKeys.sessionIndex(sessionId), storageKey);
  }

  async removeSessionBindings(
    sessionId: string,
    storageKeys: string[],
  ): Promise<void> {
    if (storageKeys.length === 0) return;
    await this.redis.srem(
      authMobilityKeys.sessionIndex(sessionId),
      ...storageKeys,
    );
  }

  async saveOwnedBinding(args: {
    storageKey: string;
    binding: MobilityBinding;
    ownerSessionId: string;
    bindingTtlSeconds: number;
    sessionIndexTtlSeconds?: number | null;
  }): Promise<void> {
    await this.saveWithTtl(
      args.storageKey,
      args.binding,
      args.bindingTtlSeconds,
    );
    await this.addSessionBinding(args.ownerSessionId, args.storageKey);
    if (args.sessionIndexTtlSeconds) {
      await this.ensureSessionIndexTtl(
        args.ownerSessionId,
        args.sessionIndexTtlSeconds,
      );
    }
  }

  async saveOrphanedBinding(args: {
    storageKey: string;
    binding: MobilityBinding;
    previousOwnerSessionId: string;
  }): Promise<void> {
    const pipeline = this.redis.pipeline();
    this.queueSaveKeepTtl(pipeline, args.storageKey, args.binding);
    this.queueRemoveSessionBinding(
      pipeline,
      args.previousOwnerSessionId,
      args.storageKey,
    );
    await pipeline.exec();
  }

  async ensureSessionIndexTtl(
    sessionId: string,
    ttlSeconds: number,
  ): Promise<void> {
    const key = authMobilityKeys.sessionIndex(sessionId);
    const currentTtl = await this.redis.ttl(key);
    if (currentTtl < ttlSeconds) {
      await this.redis.expire(key, ttlSeconds);
    }
  }

  queueSaveWithTtl(
    pipeline: RedisPipeline,
    storageKey: string,
    binding: MobilityBinding,
    ttlSeconds: number,
  ): void {
    pipeline.set(storageKey, JSON.stringify(binding), "EX", ttlSeconds);
  }

  queueSaveKeepTtl(
    pipeline: RedisPipeline,
    storageKey: string,
    binding: MobilityBinding,
  ): void {
    pipeline.set(storageKey, JSON.stringify(binding), "KEEPTTL");
  }

  queueAddSessionBinding(
    pipeline: RedisPipeline,
    sessionId: string,
    storageKey: string,
  ): void {
    pipeline.sadd(authMobilityKeys.sessionIndex(sessionId), storageKey);
  }

  queueRemoveSessionBinding(
    pipeline: RedisPipeline,
    sessionId: string,
    storageKey: string,
  ): void {
    pipeline.srem(authMobilityKeys.sessionIndex(sessionId), storageKey);
  }

  queueExpireSessionIndex(
    pipeline: RedisPipeline,
    sessionId: string,
    ttlSeconds: number,
  ): void {
    pipeline.expire(authMobilityKeys.sessionIndex(sessionId), ttlSeconds);
  }

  queueClearBinding(
    pipeline: RedisPipeline,
    subjectType: MobilitySubjectType,
    subjectKey: string,
  ): void {
    pipeline.del(this.storageKey(subjectType, subjectKey));
  }

  queueClearSessionIndex(pipeline: RedisPipeline, sessionId: string): void {
    pipeline.del(authMobilityKeys.sessionIndex(sessionId));
  }
}
