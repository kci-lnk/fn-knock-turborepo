import { randomBytes } from "node:crypto";
import type Redis from "ioredis";
import { normalizeTotpAccessScopes } from "../totp-access-scopes";
import { normalizeTotpSubdomainAccess } from "../totp-subdomain-access";
import { redisT } from "./messages";
import type { LoginSession, PasskeyCredential, TOTPCredential } from "./types";

const normalizeTOTPCredential = (value: unknown): TOTPCredential | null => {
  if (!value || typeof value !== "object") return null;
  const raw = value as Partial<TOTPCredential>;
  const id = String(raw.id ?? "").trim();
  const secret = String(raw.secret ?? "").trim();
  if (!id || !secret) return null;

  return {
    id,
    secret,
    comment: String(raw.comment ?? "").trim(),
    createdAt: String(raw.createdAt ?? "").trim() || new Date().toISOString(),
    access_scopes: normalizeTotpAccessScopes(raw.access_scopes),
    subdomain_access: normalizeTotpSubdomainAccess(raw.subdomain_access),
  };
};

const normalizeTOTPCredentials = (value: unknown): TOTPCredential[] => {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => normalizeTOTPCredential(item))
    .filter((item): item is TOTPCredential => item !== null);
};

export class AuthCredentialStore {
  private totpKey = "fn_knock:totp_secret";
  private totpListKey = "fn_knock:totps";
  private passkeyListKey = "fn_knock:passkeys";
  private passkeyChallengeKey = "fn_knock:passkey:challenge";
  private passkeyBindKey = "fn_knock:passkey:bind";

  private consumeMatchingValueScript = `
local key = KEYS[1]
local expected = ARGV[1]
local actual = redis.call('GET', key)

if not actual then
  return 0
end

if actual ~= expected then
  return -1
end

redis.call('DEL', key)
return 1
`;
  private consumeStoredValueScript = `
local key = KEYS[1]
local actual = redis.call('GET', key)

if not actual then
  return false
end

redis.call('DEL', key)
return actual
`;

  constructor(private redis: Redis) {}

  async getTOTPCredentials(): Promise<TOTPCredential[]> {
    const raw = await this.redis.get(this.totpListKey);
    if (!raw) {
      // Migration for old single secret
      const oldSecret = await this.redis.get(this.totpKey);
      if (oldSecret) {
        const legacyTotp: TOTPCredential = {
          id: "legacy-totp-id",
          secret: oldSecret,
          comment: redisT("defaultCredential"),
          createdAt: new Date().toISOString(),
          access_scopes: [],
          subdomain_access: normalizeTotpSubdomainAccess(null),
        };
        await this.saveTOTPCredentials([legacyTotp]);
        await this.redis.del(this.totpKey);
        const passkeys = await this.getPasskeys();
        let passkeysModified = false;
        for (const pk of passkeys) {
          if (!pk.totpId) {
            pk.totpId = legacyTotp.id;
            passkeysModified = true;
          }
        }
        if (passkeysModified) await this.savePasskeys(passkeys);
        return [legacyTotp];
      }
      return [];
    }
    try {
      const parsed = JSON.parse(raw);
      return normalizeTOTPCredentials(parsed);
    } catch {
      return [];
    }
  }

  async saveTOTPCredentials(totps: TOTPCredential[]): Promise<void> {
    await this.redis.set(
      this.totpListKey,
      JSON.stringify(normalizeTOTPCredentials(totps)),
    );
  }

  async addTOTPCredential(totp: TOTPCredential): Promise<void> {
    const totps = await this.getTOTPCredentials();
    const normalized = normalizeTOTPCredential(totp);
    if (!normalized) return;
    totps.push(normalized);
    await this.saveTOTPCredentials(totps);
  }

  async updateTOTPCredential(id: string, comment: string): Promise<boolean> {
    const totps = await this.getTOTPCredentials();
    const target = totps.find((t) => t.id === id);
    if (!target) return false;
    target.comment = comment;
    await this.saveTOTPCredentials(totps);
    return true;
  }

  async updateTOTPCredentialAccessScopes(
    id: string,
    accessScopes: unknown,
  ): Promise<TOTPCredential | null> {
    const totps = await this.getTOTPCredentials();
    const target = totps.find((t) => t.id === id);
    if (!target) return null;
    target.access_scopes = normalizeTotpAccessScopes(accessScopes);
    await this.saveTOTPCredentials(totps);
    return target;
  }

  async updateTOTPCredentialSubdomainAccess(
    id: string,
    subdomainAccess: unknown,
  ): Promise<TOTPCredential | null> {
    const totps = await this.getTOTPCredentials();
    const target = totps.find((t) => t.id === id);
    if (!target) return null;
    target.subdomain_access = normalizeTotpSubdomainAccess(subdomainAccess);
    await this.saveTOTPCredentials(totps);
    return target;
  }

  async deleteTOTPCredential(id: string): Promise<boolean> {
    const totps = await this.getTOTPCredentials();
    const updated = totps.filter((t) => t.id !== id);
    if (updated.length === totps.length) return false;
    await this.saveTOTPCredentials(updated);

    const passkeys = await this.getPasskeys();
    const remainingPasskeys = passkeys.filter((pk) => pk.totpId !== id);
    if (remainingPasskeys.length !== passkeys.length) {
      await this.savePasskeys(remainingPasskeys);
    }
    return true;
  }

  async addSession(
    sessionId: string,
    session: LoginSession,
    ttlSeconds: number,
  ): Promise<void> {
    await this.redis.set(
      `fn_knock:session:${sessionId}`,
      JSON.stringify(session),
      "EX",
      ttlSeconds,
    );
  }

  async getSession(sessionId: string): Promise<LoginSession | null> {
    const raw = await this.redis.get(`fn_knock:session:${sessionId}`);
    if (!raw) return null;
    try {
      const data = JSON.parse(raw) as LoginSession;
      return data;
    } catch {
      return null;
    }
  }

  async deleteSession(sessionId: string): Promise<void> {
    await this.redis.del(`fn_knock:session:${sessionId}`);
  }

  async updateSession(
    sessionId: string,
    updates: Partial<LoginSession>,
  ): Promise<LoginSession | null> {
    const key = `fn_knock:session:${sessionId}`;
    const [raw, ttl] = await Promise.all([
      this.redis.get(key),
      this.redis.ttl(key),
    ]);
    if (!raw) return null;

    try {
      const current = JSON.parse(raw) as LoginSession;
      const next: LoginSession = {
        ...current,
        ...updates,
      };

      if (ttl > 0) {
        await this.redis.set(key, JSON.stringify(next), "EX", ttl);
      } else {
        await this.redis.set(key, JSON.stringify(next));
      }
      return next;
    } catch {
      return null;
    }
  }

  async isValidSession(sessionId: string): Promise<boolean> {
    const val = await this.redis.get(`fn_knock:session:${sessionId}`);
    return val !== null;
  }

  async listSessions(): Promise<Array<{ id: string; data: LoginSession }>> {
    const match = "fn_knock:session:*";
    let cursor = "0";
    const keys: string[] = [];
    do {
      const res = await this.redis.scan(cursor, "MATCH", match, "COUNT", 100);
      cursor = res[0];
      const batch = res[1] as string[];
      if (batch && batch.length) keys.push(...batch);
    } while (cursor !== "0");
    if (keys.length === 0) return [];
    const values = await this.redis.mget(keys);
    const list: Array<{ id: string; data: LoginSession }> = [];
    keys.forEach((key, idx) => {
      const raw = values[idx];
      if (!raw) return;
      try {
        const data = JSON.parse(raw) as LoginSession;
        const id = key.replace("fn_knock:session:", "");
        list.push({ id, data });
      } catch {}
    });
    return list.sort((a, b) => {
      const at = Date.parse(a.data.loginTime) || 0;
      const bt = Date.parse(b.data.loginTime) || 0;
      return bt - at;
    });
  }

  async getPasskeys(): Promise<PasskeyCredential[]> {
    const raw = await this.redis.get(this.passkeyListKey);
    if (!raw) return [];
    try {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) return parsed as PasskeyCredential[];
    } catch {
      return [];
    }
    return [];
  }

  async savePasskeys(passkeys: PasskeyCredential[]): Promise<void> {
    await this.redis.set(this.passkeyListKey, JSON.stringify(passkeys));
  }

  async addPasskey(passkey: PasskeyCredential): Promise<void> {
    const passkeys = await this.getPasskeys();
    passkeys.push(passkey);
    await this.savePasskeys(passkeys);
  }

  async deletePasskey(id: string): Promise<boolean> {
    const passkeys = await this.getPasskeys();
    const updated = passkeys.filter((passkey) => passkey.id !== id);
    if (updated.length === passkeys.length) return false;
    await this.savePasskeys(updated);
    return true;
  }

  async updatePasskeyCounter(
    id: string,
    counter: number,
    lastUsedAt: string,
  ): Promise<boolean> {
    const passkeys = await this.getPasskeys();
    const target = passkeys.find((passkey) => passkey.id === id);
    if (!target) return false;
    target.counter = counter;
    target.lastUsedAt = lastUsedAt;
    await this.savePasskeys(passkeys);
    return true;
  }

  async setPasskeyChallenge(
    challenge: string,
    type: "register" | "auth",
    ttlSeconds: number = 300,
  ): Promise<void> {
    await this.redis.set(
      `${this.passkeyChallengeKey}:${challenge}`,
      type,
      "EX",
      ttlSeconds,
    );
  }

  async consumePasskeyChallenge(
    challenge: string,
    type: "register" | "auth",
  ): Promise<boolean> {
    const key = `${this.passkeyChallengeKey}:${challenge}`;
    const result = await this.redis.eval(
      this.consumeMatchingValueScript,
      1,
      key,
      type,
    );
    return Number(result) === 1;
  }

  async createPasskeyBindToken(
    totpId: string,
    ttlSeconds: number = 600,
  ): Promise<string> {
    const token = randomBytes(24).toString("hex");
    await this.redis.set(
      `${this.passkeyBindKey}:${token}`,
      totpId,
      "EX",
      ttlSeconds,
    );
    return token;
  }

  async isPasskeyBindTokenValid(token: string): Promise<boolean> {
    const value = await this.redis.get(`${this.passkeyBindKey}:${token}`);
    return value !== null;
  }

  async consumePasskeyBindToken(token: string): Promise<string | null> {
    const key = `${this.passkeyBindKey}:${token}`;
    const value = await this.redis.eval(this.consumeStoredValueScript, 1, key);
    return typeof value === "string" && value ? value : null;
  }
}
