import type Redis from "ioredis";

export class RedisEphemeralStore {
  constructor(private readonly redis: Redis) {}

  async addIPBackoff(ip: string, ttlSeconds: number): Promise<void> {
    await this.redis.set(this.ipBackoffKey(ip), "1", "EX", ttlSeconds);
  }

  async getIPBackoff(ip: string): Promise<boolean> {
    const value = await this.redis.get(this.ipBackoffKey(ip));
    return value !== null;
  }

  async addNonce(nonce: string, ttlSeconds = 300): Promise<void> {
    await this.redis.set(this.nonceKey(nonce), "1", "EX", ttlSeconds);
  }

  /**
   * Stores a nonce if it doesn't exist. Returns true if it was set.
   */
  async setNonceIfNotExists(nonce: string, ttlSeconds = 600): Promise<boolean> {
    const result = await this.redis.set(
      this.nonceKey(nonce),
      "1",
      "EX",
      ttlSeconds,
      "NX",
    );
    return result === "OK";
  }

  /**
   * Stores a distributed lock if it doesn't exist. Returns true when acquired.
   */
  async setLockIfNotExists(
    lockName: string,
    ttlSeconds = 600,
  ): Promise<boolean> {
    const result = await this.redis.set(
      this.lockKey(lockName),
      "1",
      "EX",
      ttlSeconds,
      "NX",
    );
    return result === "OK";
  }

  private ipBackoffKey(ip: string): string {
    return `fn_knock:backoff:${ip}`;
  }

  private nonceKey(nonce: string): string {
    return `fn_knock:nonce:${nonce}`;
  }

  private lockKey(lockName: string): string {
    return `fn_knock:lock:${lockName}`;
  }
}
