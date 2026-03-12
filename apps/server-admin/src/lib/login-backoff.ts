import { redis } from "./redis";

type AttemptState = {
  ip: string;
  attempts: number;
  lastAttempt: number;
  blockedUntil?: number;
};

type BackoffStatus = {
  ip: string;
  attempts: number;
  blocked: boolean;
  retryAfter?: number;
  blockedUntil?: number;
};

class LoginBackoffService {
  private keyPrefix = "fn_knock:login_backoff:";
  private baseDelay = 2000;
  private maxDelay = 3600000;
  private maxAttempts = 8;
  private jitterFactor = 0.4;
  private ttlSeconds = 86400;

  private key(ip: string) {
    return `${this.keyPrefix}${ip}`;
  }

  private async get(ip: string): Promise<AttemptState | null> {
    const raw = await redis.get(this.key(ip));
    if (!raw) return null;
    try {
      return JSON.parse(raw) as AttemptState;
    } catch {
      return null;
    }
  }

  private async set(ip: string, state: AttemptState) {
    await redis.set(this.key(ip), JSON.stringify(state), "EX", this.ttlSeconds);
  }

  private jitter(delay: number) {
    const max = delay * this.jitterFactor;
    return (Math.random() * 2 - 1) * max;
  }

  private calcBackoff(attempts: number) {
    if (attempts <= 0) return 0;
    const exp = Math.pow(2, attempts - 1) * this.baseDelay;
    const v = exp + this.jitter(exp);
    return Math.min(Math.max(0, Math.floor(v)), this.maxDelay);
  }

  async getStatus(ip: string): Promise<BackoffStatus> {
    const s = await this.get(ip);
    if (!s) return { ip, attempts: 0, blocked: false };
    const now = Date.now();
    const blocked = s.blockedUntil ? now < s.blockedUntil : false;
    const retryAfter = blocked && s.blockedUntil ? Math.ceil((s.blockedUntil - now) / 1000) : undefined;
    return { ip, attempts: s.attempts, blocked, retryAfter, blockedUntil: s.blockedUntil };
  }

  async ensureNotBlocked(ip: string): Promise<{ allowed: boolean; retryAfter?: number }> {
    const st = await this.getStatus(ip);
    if (st.blocked && st.retryAfter && st.retryAfter > 0) {
      return { allowed: false, retryAfter: st.retryAfter };
    }
    return { allowed: true };
  }

  async registerFailure(ip: string): Promise<{ retryAfter: number }> {
    const existing = await this.get(ip);
    const now = Date.now();
    const next: AttemptState = existing
      ? { ...existing, attempts: existing.attempts + 1, lastAttempt: now }
      : { ip, attempts: 1, lastAttempt: now };
    const backoffMs = this.calcBackoff(next.attempts);
    next.blockedUntil = now + backoffMs;
    await this.set(ip, next);
    return { retryAfter: Math.ceil(backoffMs / 1000) };
  }

  async reset(ip: string): Promise<void> {
    await redis.del(this.key(ip));
  }

  async listBlocked(): Promise<BackoffStatus[]> {
    const pattern = `${this.keyPrefix}*`;
    let cursor = "0";
    const keys: string[] = [];
    do {
      const res = await redis.scan(cursor, "MATCH", pattern, "COUNT", 100);
      cursor = res[0];
      const batch = res[1] as string[];
      if (batch && batch.length) keys.push(...batch);
    } while (cursor !== "0");
    if (keys.length === 0) return [];
    const vals = await redis.mget(keys);
    const now = Date.now();
    const items: BackoffStatus[] = [];
    for (let i = 0; i < keys.length; i++) {
      const raw = vals[i];
      if (!raw) continue;
      try {
        const s = JSON.parse(raw) as AttemptState;
        const blocked = s.blockedUntil ? now < s.blockedUntil : false;
        const retryAfter = blocked && s.blockedUntil ? Math.ceil((s.blockedUntil - now) / 1000) : undefined;
        if (blocked) {
          const keyStr = keys[i] ?? "";
          const ip = keyStr.slice(this.keyPrefix.length);
          items.push({ ip, attempts: s.attempts, blocked, retryAfter, blockedUntil: s.blockedUntil });
        }
      } catch {
        continue;
      }
    }
    items.sort((a, b) => (b.retryAfter || 0) - (a.retryAfter || 0));
    return items;
  }

  shouldHardBlock(attempts: number) {
    return attempts >= this.maxAttempts;
  }
}

export const loginBackoffService = new LoginBackoffService();
