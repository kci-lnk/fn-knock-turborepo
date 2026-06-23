import { Elysia } from "elysia";
import { isIP } from "node:net";
import { ddnsFetch } from "../../lib/ddns/network";
import {
  DEFAULT_DDNS_PUBLIC_CHECK_SOURCES,
  normalizeDDNSPublicCheckSources,
} from "../../lib/ddns/public-check-sources";
import { ddnsTranslate } from "../../lib/ddns/providers/helpers";
import type {
  DDNSPublicCheckFamily,
  DDNSPublicCheckSources,
  DDNSPublicCheckTestResult,
} from "../../lib/ddns/types";

const IP_DETECTION_TIMEOUT_MS = 7000;
const RESPONSE_PREVIEW_MAX_LENGTH = 240;
const ddnsT = ddnsTranslate;

export class IPDetector {
  private static parseDetectedIP(value: unknown, family: 4 | 6): string | null {
    const candidate = String(value ?? "").trim();
    return isIP(candidate) === family ? candidate : null;
  }

  private static parseDetectedIPText(text: string, family: 4 | 6): string | null {
    const plainTextIP = this.parseDetectedIP(text, family);
    if (plainTextIP) {
      return plainTextIP;
    }

    try {
      const data = JSON.parse(text);
      return this.parseDetectedIP(data.ip || data.address || data, family);
    } catch {
      return null;
    }
  }

  private static getFamilyVersion(family: DDNSPublicCheckFamily): 4 | 6 {
    return family === "ipv4" ? 4 : 6;
  }

  private static getFamilyLabel(family: DDNSPublicCheckFamily): string {
    return family === "ipv4" ? "IPv4" : "IPv6";
  }

  private static trimResponsePreview(text: string): string {
    const preview = text.trim().replace(/\s+/g, " ");
    return preview.length > RESPONSE_PREVIEW_MAX_LENGTH
      ? `${preview.slice(0, RESPONSE_PREVIEW_MAX_LENGTH)}...`
      : preview;
  }

  private static async testSingleSource(
    url: string,
    family: DDNSPublicCheckFamily,
    options: { networkInterface?: string | null } = {},
  ): Promise<DDNSPublicCheckTestResult> {
    const preferredFamily = this.getFamilyVersion(family);

    try {
      const res = await ddnsFetch(url, {
        networkInterface: options.networkInterface,
        preferredFamily,
        signal: AbortSignal.timeout(IP_DETECTION_TIMEOUT_MS),
        headers: { Accept: "application/json, text/plain" },
      });
      const text = await res.text();
      const responsePreview = this.trimResponsePreview(text);

      if (!res.ok) {
        return {
          family,
          url,
          success: false,
          status: res.status,
          ip: null,
          responsePreview,
          error: ddnsT("publicCheckSourceRequestFailed", {
            url,
            status: res.status,
          }),
        };
      }

      const ip = this.parseDetectedIPText(text, preferredFamily);
      if (!ip) {
        return {
          family,
          url,
          success: false,
          status: res.status,
          ip: null,
          responsePreview,
          error: ddnsT("publicCheckSourceInvalidPayload", {
            url,
            family: this.getFamilyLabel(family),
          }),
        };
      }

      return {
        family,
        url,
        success: true,
        status: res.status,
        ip,
        responsePreview,
      };
    } catch (error) {
      return {
        family,
        url,
        success: false,
        status: null,
        ip: null,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }

  private static async raceSources(
    sources: string[],
    options: {
      family: DDNSPublicCheckFamily;
      networkInterface?: string | null;
    },
  ): Promise<string> {
    if (sources.length === 0) {
      throw new Error(
        ddnsT("publicCheckSourceListEmpty", {
          family: this.getFamilyLabel(options.family),
        }),
      );
    }

    const failures: string[] = [];
    const fetchTasks = sources.map(async (url) => {
      const result = await this.testSingleSource(url, options.family, {
        networkInterface: options.networkInterface,
      });
      if (result.success && result.ip) {
        return result.ip;
      }
      failures.push(result.error || `Source ${url} failed`);
      throw new Error(result.error || `Source ${url} failed`);
    });

    try {
      return await Promise.any(fetchTasks);
    } catch {
      throw new Error(failures.filter(Boolean).join("; "));
    }
  }

  private static async detectFamily(
    sources: string[],
    options: {
      family: DDNSPublicCheckFamily;
      networkInterface?: string | null;
    },
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
    publicCheckSources?: DDNSPublicCheckSources;
  } = {}) {
    const enableIPv4 = options.enableIPv4 !== false;
    const enableIPv6 = options.enableIPv6 !== false;
    const publicCheckSources =
      options.publicCheckSources || DEFAULT_DDNS_PUBLIC_CHECK_SOURCES;
    const [ipv4Result, ipv6Result] = await Promise.all([
      enableIPv4
        ? this.detectFamily(publicCheckSources.ipv4, {
            ...options,
            family: "ipv4",
          })
        : Promise.resolve({ ip: null, error: null }),
      enableIPv6
        ? this.detectFamily(publicCheckSources.ipv6, {
            ...options,
            family: "ipv6",
          })
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

  static async testPublicCheckSources(
    sources: DDNSPublicCheckSources,
    options: {
      networkInterface?: string | null;
    } = {},
  ): Promise<DDNSPublicCheckTestResult[]> {
    const normalized = normalizeDDNSPublicCheckSources(sources, {
      ipv4: [],
      ipv6: [],
    });
    const tests = [
      ...normalized.ipv4.map((url) =>
        this.testSingleSource(url, "ipv4", options),
      ),
      ...normalized.ipv6.map((url) =>
        this.testSingleSource(url, "ipv6", options),
      ),
    ];

    return Promise.all(tests);
  }
}

export const ipDetectorPlugin = new Elysia({ name: "plugin-ip-detector" })
  .decorate("ipDetector", IPDetector);
