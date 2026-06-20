import { tDefault } from "../i18n";

type RedisMessageParams = Record<
  string,
  string | number | boolean | null | undefined
>;

export const redisT = (key: string, params?: RedisMessageParams) =>
  tDefault(`server.redis.${key}`, params);
