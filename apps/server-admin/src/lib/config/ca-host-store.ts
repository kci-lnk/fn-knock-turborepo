import type Redis from "ioredis";

export class CaHostStore {
  private readonly key = "fn_knock:ca:hosts";

  constructor(private readonly redis: Redis) {}

  async getHosts(): Promise<string[]> {
    const raw = await this.redis.get(this.key);
    if (!raw) return [];
    try {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) {
        return parsed.filter((item) => typeof item === "string");
      }
    } catch {}
    return [];
  }

  async saveHosts(hosts: string[]): Promise<void> {
    await this.redis.set(this.key, JSON.stringify(hosts));
  }

  async addHost(value: string): Promise<string[]> {
    const host = value.trim();
    if (!host) return this.getHosts();

    const hosts = await this.getHosts();
    if (!hosts.includes(host)) {
      hosts.push(host);
      await this.saveHosts(hosts);
    }
    return hosts;
  }

  async removeHost(value: string): Promise<string[]> {
    const host = value.trim();
    const hosts = await this.getHosts();
    const next = hosts.filter((item) => item !== host);
    if (next.length !== hosts.length) {
      await this.saveHosts(next);
    }
    return next;
  }

  async clearHosts(): Promise<void> {
    await this.saveHosts([]);
  }
}
