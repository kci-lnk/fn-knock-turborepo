import type Redis from "ioredis";
import {
  normalizeDDNSConfig,
  prepareDDNSConfigForStorage,
} from "./config-normalizer";
import {
  parseDDNSLastCheckHash,
  parseDDNSLastIPHash,
  serializeDDNSLastCheckHash,
  serializeDDNSLastIPHash,
} from "./status-codec";
import type {
  DDNSLastCheck,
  DDNSLastIP,
  DDNSStoredSettings,
  DDNSTargetMeta,
} from "./types";

export const PRIMARY_DDNS_TARGET_ID = "primary";

export const DDNS_REDIS_KEYS = {
  enabled: "fn_knock:ddns:enabled",
  settings: "fn_knock:ddns:settings",
  legacyProvider: "fn_knock:ddns:provider",
  legacyConfigPrefix: "fn_knock:ddns:config:",
  legacyLastIP: "fn_knock:ddns:last_ip",
  legacyLastCheck: "fn_knock:ddns:last_check",
  targetIds: "fn_knock:ddns:v2:target_ids",
  primaryTargetId: "fn_knock:ddns:v2:primary_target_id",
  targetPrefix: "fn_knock:ddns:v2:target:",
  logs: "fn_knock:ddns:logs",
  logSeq: "fn_knock:ddns:logs:seq",
} as const;

const targetMetaKey = (id: string) =>
  `${DDNS_REDIS_KEYS.targetPrefix}${id}:meta`;
const targetConfigKey = (id: string) =>
  `${DDNS_REDIS_KEYS.targetPrefix}${id}:config`;
const targetLastIPKey = (id: string) =>
  `${DDNS_REDIS_KEYS.targetPrefix}${id}:last_ip`;
const targetLastCheckKey = (id: string) =>
  `${DDNS_REDIS_KEYS.targetPrefix}${id}:last_check`;

const parseTargetMeta = (
  id: string,
  data: Record<string, string> | null | undefined,
  primaryTargetName: string,
): DDNSTargetMeta | null => {
  if (!data || Object.keys(data).length === 0) {
    return null;
  }

  const now = new Date().toISOString();
  const parsedSortOrder = Number(data.sort_order);

  return {
    id,
    name:
      data.name?.trim() ||
      (id === PRIMARY_DDNS_TARGET_ID ? primaryTargetName : ""),
    isPrimary: data.is_primary === "true" || id === PRIMARY_DDNS_TARGET_ID,
    enabled: id === PRIMARY_DDNS_TARGET_ID ? true : data.enabled !== "false",
    provider: data.provider?.trim() || null,
    createdAt: data.created_at || now,
    updatedAt: data.updated_at || data.created_at || now,
    sortOrder: Number.isFinite(parsedSortOrder)
      ? parsedSortOrder
      : id === PRIMARY_DDNS_TARGET_ID
        ? 0
        : 1,
  };
};

export class DDNSRedisStore {
  constructor(private readonly redis: Redis) {}

  async getEnabled(): Promise<boolean> {
    return (await this.redis.get(DDNS_REDIS_KEYS.enabled)) === "true";
  }

  async setEnabled(enabled: boolean): Promise<void> {
    await this.redis.set(DDNS_REDIS_KEYS.enabled, enabled ? "true" : "false");
  }

  async getSettingsRaw(): Promise<string | null> {
    return this.redis.get(DDNS_REDIS_KEYS.settings);
  }

  async saveSettings(settings: DDNSStoredSettings): Promise<void> {
    await this.redis.set(DDNS_REDIS_KEYS.settings, JSON.stringify(settings));
  }

  async getPrimaryTargetId(): Promise<string | null> {
    return this.redis.get(DDNS_REDIS_KEYS.primaryTargetId);
  }

  async addTargetId(targetId: string): Promise<void> {
    await this.redis.sadd(DDNS_REDIS_KEYS.targetIds, targetId);
  }

  async listTargetIds(): Promise<string[]> {
    return this.redis.smembers(DDNS_REDIS_KEYS.targetIds);
  }

  async getTargetMeta(
    id: string,
    primaryTargetName: string,
  ): Promise<DDNSTargetMeta | null> {
    return parseTargetMeta(
      id,
      await this.redis.hgetall(targetMetaKey(id)),
      primaryTargetName,
    );
  }

  async saveTargetMeta(meta: DDNSTargetMeta): Promise<void> {
    const key = targetMetaKey(meta.id);
    const payload: Record<string, string> = {
      name: meta.name.trim(),
      is_primary: meta.isPrimary ? "true" : "false",
      enabled: meta.enabled ? "true" : "false",
      provider: meta.provider?.trim() || "",
      created_at: meta.createdAt,
      updated_at: meta.updatedAt,
      sort_order: String(meta.sortOrder),
    };

    await this.redis.del(key);
    await this.redis.hmset(key, payload);
    await this.addTargetId(meta.id);
    if (meta.isPrimary) {
      await this.redis.set(DDNS_REDIS_KEYS.primaryTargetId, meta.id);
    }
  }

  async getTargetConfig(
    id: string,
    providerName: string | null | undefined,
  ): Promise<Record<string, string>> {
    return normalizeDDNSConfig(
      providerName,
      await this.redis.hgetall(targetConfigKey(id)),
    );
  }

  async saveTargetConfig(
    id: string,
    providerName: string | null | undefined,
    config: Record<string, string>,
  ): Promise<void> {
    const key = targetConfigKey(id);
    const payload = prepareDDNSConfigForStorage(providerName, config);
    await this.redis.del(key);
    if (Object.keys(payload).length > 0) {
      await this.redis.hmset(key, payload as Record<string, string>);
    }
  }

  async getTargetLastIP(id: string): Promise<DDNSLastIP> {
    return parseDDNSLastIPHash(await this.redis.hgetall(targetLastIPKey(id)));
  }

  async saveTargetLastIP(id: string, status: DDNSLastIP): Promise<void> {
    const key = targetLastIPKey(id);
    const payload = serializeDDNSLastIPHash(status);
    await this.redis.del(key);
    if (Object.keys(payload).length > 0) {
      await this.redis.hmset(key, payload);
    }
  }

  async getTargetLastCheck(id: string): Promise<DDNSLastCheck> {
    return parseDDNSLastCheckHash(
      await this.redis.hgetall(targetLastCheckKey(id)),
    );
  }

  async saveTargetLastCheck(
    id: string,
    status: DDNSLastCheck,
  ): Promise<void> {
    const key = targetLastCheckKey(id);
    const payload = serializeDDNSLastCheckHash(status);
    await this.redis.del(key);
    if (Object.keys(payload).length > 0) {
      await this.redis.hmset(key, payload);
    }
  }

  async deleteTarget(targetId: string): Promise<void> {
    await Promise.all([
      this.redis.srem(DDNS_REDIS_KEYS.targetIds, targetId),
      this.redis.del(targetMetaKey(targetId)),
      this.redis.del(targetConfigKey(targetId)),
      this.redis.del(targetLastIPKey(targetId)),
      this.redis.del(targetLastCheckKey(targetId)),
    ]);
  }

  async readLegacyProvider(): Promise<string | null> {
    return (await this.redis.get(DDNS_REDIS_KEYS.legacyProvider))?.trim() || null;
  }

  async mirrorPrimaryProvider(
    providerName: string | null | undefined,
  ): Promise<void> {
    const normalizedProviderName = providerName?.trim() || "";
    if (!normalizedProviderName) {
      await this.redis.del(DDNS_REDIS_KEYS.legacyProvider);
      return;
    }
    await this.redis.set(DDNS_REDIS_KEYS.legacyProvider, normalizedProviderName);
  }

  async readLegacyConfigDraft(
    providerName: string | null | undefined,
  ): Promise<Record<string, string>> {
    const normalizedProviderName = providerName?.trim() || "";
    if (!normalizedProviderName) {
      return normalizeDDNSConfig(null, {});
    }

    return normalizeDDNSConfig(
      normalizedProviderName,
      await this.redis.hgetall(
        DDNS_REDIS_KEYS.legacyConfigPrefix + normalizedProviderName,
      ),
    );
  }

  async saveLegacyConfigDraft(
    providerName: string | null | undefined,
    config: Record<string, string>,
  ): Promise<void> {
    const normalizedProviderName = providerName?.trim() || "";
    if (!normalizedProviderName) {
      return;
    }

    const key = DDNS_REDIS_KEYS.legacyConfigPrefix + normalizedProviderName;
    const payload = prepareDDNSConfigForStorage(
      normalizedProviderName,
      config,
    );
    await this.redis.del(key);
    if (Object.keys(payload).length > 0) {
      await this.redis.hmset(key, payload as Record<string, string>);
    }
  }

  async readLegacyLastIP(): Promise<DDNSLastIP> {
    return parseDDNSLastIPHash(
      await this.redis.hgetall(DDNS_REDIS_KEYS.legacyLastIP),
    );
  }

  async writeLegacyLastIP(status: DDNSLastIP): Promise<void> {
    const payload = serializeDDNSLastIPHash(status);
    await this.redis.del(DDNS_REDIS_KEYS.legacyLastIP);
    if (Object.keys(payload).length > 0) {
      await this.redis.hmset(DDNS_REDIS_KEYS.legacyLastIP, payload);
    }
  }

  async readLegacyLastCheck(): Promise<DDNSLastCheck> {
    return parseDDNSLastCheckHash(
      await this.redis.hgetall(DDNS_REDIS_KEYS.legacyLastCheck),
    );
  }

  async writeLegacyLastCheck(status: DDNSLastCheck): Promise<void> {
    const payload = serializeDDNSLastCheckHash(status);
    await this.redis.del(DDNS_REDIS_KEYS.legacyLastCheck);
    if (Object.keys(payload).length > 0) {
      await this.redis.hmset(DDNS_REDIS_KEYS.legacyLastCheck, payload);
    }
  }
}
