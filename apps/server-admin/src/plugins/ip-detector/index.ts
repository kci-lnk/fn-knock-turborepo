import { Elysia } from "elysia";
import { isIP } from "node:net";
import { ddnsFetch } from "../../lib/ddns/network";

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

  private static parseDetectedIP(value: unknown, family: 4 | 6): string | null {
    const candidate = String(value ?? "").trim();
    return isIP(candidate) === family ? candidate : null;
  }

  private static async raceSources(
    sources: string[],
    options: { networkInterface?: string | null; preferredFamily?: 4 | 6 } = {},
  ): Promise<string> {
    const fetchTasks = sources.map(async (url) => {
      const res = await ddnsFetch(url, {
        networkInterface: options.networkInterface,
        preferredFamily: options.preferredFamily,
        signal: AbortSignal.timeout(3000),
        headers: { Accept: 'application/json, text/plain' },
      });

      if (!res.ok) throw new Error(`Source ${url} failed`);

      const text = await res.text();
      try {
        const data = JSON.parse(text);
        const parsed = this.parseDetectedIP(data.ip || data.address || data, options.preferredFamily ?? 4);
        if (!parsed) {
          throw new Error(`Source ${url} returned invalid IP payload`);
        }
        return parsed;
      } catch {
        const parsed = this.parseDetectedIP(text, options.preferredFamily ?? 4);
        if (!parsed) {
          throw new Error(`Source ${url} returned invalid IP text`);
        }
        return parsed;
      }
    });
    return Promise.any(fetchTasks);
  }

  private static async detectFamily(
    sources: string[],
    options: { networkInterface?: string | null; preferredFamily: 4 | 6 },
  ): Promise<{ ip: string | null; error: string | null }> {
    try {
      const ip = await this.raceSources(sources, options);
      return { ip, error: null };
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return { ip: null, error: message };
    }
  }

  static async getCurrentIPs(options: {
    networkInterface?: string | null;
    enableIPv4?: boolean;
    enableIPv6?: boolean;
  } = {}) {
    const enableIPv4 = options.enableIPv4 !== false;
    const enableIPv6 = options.enableIPv6 !== false;
    const [ipv4Result, ipv6Result] = await Promise.all([
      enableIPv4
        ? this.detectFamily(this.V4_SOURCES, { ...options, preferredFamily: 4 })
        : Promise.resolve({ ip: null, error: null }),
      enableIPv6
        ? this.detectFamily(this.V6_SOURCES, { ...options, preferredFamily: 6 })
        : Promise.resolve({ ip: null, error: null }),
    ]);

    return {
      ipv4: ipv4Result.ip,
      ipv6: ipv6Result.ip,
      errors: {
        ipv4: ipv4Result.error,
        ipv6: ipv6Result.error,
      },
    };
  }
}

export const ipDetectorPlugin = new Elysia({ name: 'plugin-ip-detector' })
  .decorate('ipDetector', IPDetector)
