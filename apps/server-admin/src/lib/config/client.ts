import Redis, { type RedisOptions } from "ioredis";

type RedisError = Error & {
  code?: string;
};

type WaitForRedisOptions = {
  timeoutMs?: number;
  retryDelayMs?: number;
};

const parsePositiveInteger = (
  value: string | undefined,
  fallback: number,
): number => {
  const parsed = Number.parseInt(value || "", 10);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback;
};

const parseStartupWaitMs = (): number => {
  const waitMs = parsePositiveInteger(process.env.REDIS_STARTUP_WAIT_MS, 0);
  if (waitMs > 0) return waitMs;

  const waitSeconds = parsePositiveInteger(
    process.env.REDIS_STARTUP_WAIT_SECONDS,
    0,
  );
  if (waitSeconds > 0) return waitSeconds * 1000;

  return 120000;
};

const parseStartupRetryDelayMs = (): number => {
  const retryDelayMs = parsePositiveInteger(
    process.env.REDIS_STARTUP_RETRY_DELAY_MS,
    0,
  );
  if (retryDelayMs > 0) return retryDelayMs;

  const retryDelaySeconds = parsePositiveInteger(
    process.env.REDIS_STARTUP_RETRY_DELAY_SECONDS,
    0,
  );
  if (retryDelaySeconds > 0) return retryDelaySeconds * 1000;

  return 1000;
};

const resolveRedisHost = (): string => {
  const host = process.env.REDIS_HOST?.trim();
  if (host) return host;
  return process.env.FN_KNOCK_RUNTIME_TARGET === "docker"
    ? "redis"
    : "127.0.0.1";
};

const REDIS_HOST = resolveRedisHost();
const REDIS_PORT = parsePositiveInteger(process.env.REDIS_PORT, 6379);
const REDIS_STARTUP_WAIT_MS = parseStartupWaitMs();
const REDIS_STARTUP_RETRY_DELAY_MS = parseStartupRetryDelayMs();
const REDIS_CONNECT_TIMEOUT_MS = parsePositiveInteger(
  process.env.REDIS_CONNECT_TIMEOUT_MS,
  1000,
);

const REDIS_CONFIG: RedisOptions = {
  host: REDIS_HOST,
  port: REDIS_PORT,
  password: process.env.REDIS_PASSWORD?.trim() || undefined,
  lazyConnect: true,
  connectTimeout: REDIS_CONNECT_TIMEOUT_MS,
  retryStrategy(times) {
    return Math.min(200 + times * 100, 2000);
  },
};

const startupConnectionErrorCodes = new Set([
  "ECONNREFUSED",
  "ECONNRESET",
  "ENOTFOUND",
  "EAI_AGAIN",
  "ETIMEDOUT",
]);

export const redis = new Redis(REDIS_CONFIG);

let redisStartupComplete = false;
let redisStartupWaitLogged = false;
let redisStartupPromise: Promise<void> | null = null;

const isStartupConnectionError = (error: Error): error is RedisError => {
  const code = (error as RedisError).code;
  return typeof code === "string" && startupConnectionErrorCodes.has(code);
};

redis.on("error", (error) => {
  if (!redisStartupComplete && isStartupConnectionError(error)) {
    if (!redisStartupWaitLogged) {
      console.warn(`[redis] waiting for Redis at ${REDIS_HOST}:${REDIS_PORT}`);
      redisStartupWaitLogged = true;
    }
    return;
  }

  console.error("Redis connection error:", error);
});

const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

const createProbeRedisClient = (): Redis => {
  const probe = new Redis({
    ...REDIS_CONFIG,
    lazyConnect: true,
    enableOfflineQueue: false,
    maxRetriesPerRequest: 1,
    retryStrategy: () => null,
  });
  probe.on("error", () => {
    // The startup wait loop reports one concise status line instead.
  });
  return probe;
};

const probeRedis = async (): Promise<void> => {
  const probe = createProbeRedisClient();
  try {
    await probe.connect();
    await probe.ping();
  } finally {
    probe.disconnect();
  }
};

const connectSharedRedisClient = async (): Promise<void> => {
  if (redis.status === "wait" || redis.status === "end") {
    await redis.connect();
  }
  await redis.ping();
};

const waitForRedisOnce = async ({
  timeoutMs = REDIS_STARTUP_WAIT_MS,
  retryDelayMs = REDIS_STARTUP_RETRY_DELAY_MS,
}: WaitForRedisOptions): Promise<void> => {
  const startedAt = Date.now();
  let attempts = 0;
  let lastError: unknown = null;

  while (true) {
    attempts += 1;
    try {
      await probeRedis();
      await connectSharedRedisClient();
      redisStartupComplete = true;
      if (redisStartupWaitLogged || attempts > 1) {
        console.log(`[redis] connected to ${REDIS_HOST}:${REDIS_PORT}`);
      }
      return;
    } catch (error) {
      lastError = error;
      const elapsedMs = Date.now() - startedAt;
      if (elapsedMs >= timeoutMs) {
        const message =
          lastError instanceof Error ? lastError.message : String(lastError);
        throw new Error(
          `Redis at ${REDIS_HOST}:${REDIS_PORT} was not ready after ${Math.ceil(
            timeoutMs / 1000,
          )}s: ${message}`,
        );
      }

      if (!redisStartupWaitLogged) {
        console.warn(
          `[redis] waiting for Redis at ${REDIS_HOST}:${REDIS_PORT} for up to ${Math.ceil(
            timeoutMs / 1000,
          )}s`,
        );
        redisStartupWaitLogged = true;
      }

      await sleep(Math.min(retryDelayMs, timeoutMs - elapsedMs));
    }
  }
};

export const waitForRedis = (
  options: WaitForRedisOptions = {},
): Promise<void> => {
  if (redisStartupComplete) return Promise.resolve();
  if (!redisStartupPromise) {
    redisStartupPromise = waitForRedisOnce(options).catch((error) => {
      redisStartupPromise = null;
      throw error;
    });
  }
  return redisStartupPromise;
};
