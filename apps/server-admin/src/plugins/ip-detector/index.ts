import { Elysia } from "elysia";

export class IPDetector {
  private static readonly V4_SOURCES = [
    "https://api4.ipify.org?format=json",
    "https://v4.ident.me/.json",
    "https://ipv4.icanhazip.com",
    "https://4.ipw.cn",
    "https://4.fnknock.cn"
  ];

  private static readonly V6_SOURCES = [
    "https://api6.ipify.org?format=json",
    "https://v6.ident.me/.json",
    "https://ipv6.icanhazip.com",
    "https://6.ipw.cn",
    "https://6.fnknock.cn"
  ];

  private static async raceSources(sources: string[]): Promise<string> {
    const fetchTasks = sources.map(async (url) => {
      const res = await fetch(url, { 
        signal: AbortSignal.timeout(3000),
        headers: { 'Accept': 'application/json, text/plain' }
      });
      
      if (!res.ok) throw new Error(`Source ${url} failed`);
      
      const text = await res.text();
      try {
        const data = JSON.parse(text);
        return data.ip || data.address || data; 
      } catch {
        return text.trim();
      }
    });
    return Promise.any(fetchTasks);
  }

  static async getCurrentIPs() {
    const results = await Promise.allSettled([
      this.raceSources(this.V4_SOURCES),
      this.raceSources(this.V6_SOURCES)
    ]);

    return {
      ipv4: results[0].status === "fulfilled" ? results[0].value : null,
      ipv6: results[1].status === "fulfilled" ? results[1].value : null,
    };
  }
}

export const ipDetectorPlugin = new Elysia({ name: 'plugin-ip-detector' })
  .decorate('ipDetector', IPDetector)