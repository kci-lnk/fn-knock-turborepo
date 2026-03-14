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
  private registerFailureScript = `
local key = KEYS[1]
local ip = ARGV[1]
local now = tonumber(ARGV[2])
local ttlSeconds = tonumber(ARGV[3])
local baseDelay = tonumber(ARGV[4])
local maxDelay = tonumber(ARGV[5])
local jitterFactor = tonumber(ARGV[6])

local attempts = 0
local raw = redis.call('GET', key)
if raw then
  local ok, decoded = pcall(cjson.decode, raw)
  if ok and type(decoded) == 'table' and tonumber(decoded.attempts) then
    attempts = tonumber(decoded.attempts)
  end
end

attempts = attempts + 1

local expDelay = math.pow(2, attempts - 1) * baseDelay
local seed = ip .. ':' .. tostring(attempts) .. ':' .. tostring(now)
local hash = 0
for i = 1, #seed do
  hash = (hash * 33 + string.byte(seed, i)) % 1000003
end
local ratio = (hash % 10000) / 10000
local jitter = ((ratio * 2) - 1) * (expDelay * jitterFactor)
local backoffMs = math.floor(expDelay + jitter)
if backoffMs < 0 then
  backoffMs = 0
end
if backoffMs > maxDelay then
  backoffMs = maxDelay
end

local blockedUntil = now + backoffMs
local nextState = cjson.encode({
  ip = ip,
  attempts = attempts,
  lastAttempt = now,
  blockedUntil = blockedUntil,
})

redis.call('SET', key, nextState, 'EX', ttlSeconds)
return {attempts, math.ceil(backoffMs / 1000), blockedUntil}
`;

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
    const now = Date.now();
    const result = await redis.eval(
      this.registerFailureScript,
      1,
      this.key(ip),
      ip,
      String(now),
      String(this.ttlSeconds),
      String(this.baseDelay),
      String(this.maxDelay),
      String(this.jitterFactor),
    );
    const [, retryAfterRaw] = Array.isArray(result) ? result : [0, 0];
    const retryAfter = Number(retryAfterRaw);
    return { retryAfter: Number.isFinite(retryAfter) ? retryAfter : 0 };
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
