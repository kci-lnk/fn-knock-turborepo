import type Redis from "ioredis";

import type { AppConfig } from "./types";

type ConfigAccess = {
  getConfig: () => Promise<AppConfig>;
  saveConfig: (config: AppConfig) => Promise<void>;
};

export const readConfigSection = async <T>({
  access,
  normalize,
  select,
}: {
  access: ConfigAccess;
  normalize: (value: any) => T;
  select: (config: AppConfig) => unknown;
}): Promise<T> => {
  const config = await access.getConfig();
  return normalize(select(config));
};

export const updateConfigSectionPatch = async <T>({
  access,
  assign,
  normalize,
  patch,
  select,
}: {
  access: ConfigAccess;
  assign: (config: AppConfig, value: T) => void;
  normalize: (value: any) => T;
  patch: Record<string, unknown>;
  select: (config: AppConfig) => unknown;
}): Promise<T> => {
  const config = await access.getConfig();
  const next = normalize({
    ...(typeof select(config) === "object" && select(config) !== null
      ? (select(config) as Record<string, unknown>)
      : {}),
    ...patch,
  });
  assign(config, next);
  await access.saveConfig(config);
  return next;
};

export const replaceConfigSection = async <T>({
  access,
  assign,
  normalize,
  value,
}: {
  access: ConfigAccess;
  assign: (config: AppConfig, value: T) => void;
  normalize: (value: any) => T;
  value: unknown;
}): Promise<T> => {
  const config = await access.getConfig();
  const next = normalize(value);
  assign(config, next);
  await access.saveConfig(config);
  return next;
};

export const readRedisJsonValue = async <T>({
  fallback,
  key,
  normalize,
  parseErrorMessage,
  redis,
}: {
  fallback: () => T;
  key: string;
  normalize: (value: any) => T;
  parseErrorMessage?: string;
  redis: Redis;
}): Promise<T> => {
  try {
    const raw = await redis.get(key);
    if (raw) {
      return normalize(JSON.parse(raw));
    }
  } catch (error) {
    if (parseErrorMessage) {
      console.error(parseErrorMessage, error);
    }
  }

  return fallback();
};

export const saveRedisJsonValue = async <T>({
  key,
  normalize,
  redis,
  value,
}: {
  key: string;
  normalize: (value: any) => T;
  redis: Redis;
  value: unknown;
}): Promise<T> => {
  const next = normalize(value);
  await redis.set(key, JSON.stringify(next));
  return next;
};

export const updateRedisJsonPatch = async <T>({
  fallback,
  key,
  normalize,
  patch,
  redis,
}: {
  fallback: () => T;
  key: string;
  normalize: (value: any) => T;
  patch: Record<string, unknown>;
  redis: Redis;
}): Promise<T> => {
  const current = await readRedisJsonValue({
    fallback,
    key,
    normalize,
    redis,
  });
  return saveRedisJsonValue({
    key,
    normalize,
    redis,
    value: {
      ...(current as Record<string, unknown>),
      ...patch,
    },
  });
};

export class ConfigSectionStore {
  constructor(
    private readonly access: ConfigAccess,
    private readonly redis: Redis,
  ) {}

  read<T>(
    select: (config: AppConfig) => unknown,
    normalize: (value: any) => T,
  ): Promise<T> {
    return readConfigSection({
      access: this.access,
      normalize,
      select,
    });
  }

  patch<T>(
    select: (config: AppConfig) => unknown,
    assign: (config: AppConfig, value: T) => void,
    patch: Record<string, unknown>,
    normalize: (value: any) => T,
  ): Promise<T> {
    return updateConfigSectionPatch({
      access: this.access,
      assign,
      normalize,
      patch,
      select,
    });
  }

  replace<T>(
    assign: (config: AppConfig, value: T) => void,
    value: unknown,
    normalize: (value: any) => T,
  ): Promise<T> {
    return replaceConfigSection({
      access: this.access,
      assign,
      normalize,
      value,
    });
  }

  readJson<T>(
    key: string,
    normalize: (value: any) => T,
    fallback: () => T,
    parseErrorMessage?: string,
  ): Promise<T> {
    return readRedisJsonValue({
      fallback,
      key,
      normalize,
      parseErrorMessage,
      redis: this.redis,
    });
  }

  saveJson<T>(
    key: string,
    value: unknown,
    normalize: (value: any) => T,
  ): Promise<T> {
    return saveRedisJsonValue({
      key,
      normalize,
      redis: this.redis,
      value,
    });
  }

  patchJson<T>(
    key: string,
    patch: Record<string, unknown>,
    normalize: (value: any) => T,
    fallback: () => T,
  ): Promise<T> {
    return updateRedisJsonPatch({
      fallback,
      key,
      normalize,
      patch,
      redis: this.redis,
    });
  }
}
