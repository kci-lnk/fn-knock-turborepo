import type Redis from "ioredis";
import {
  DEFAULT_ACME_CERTIFICATE_AUTHORITY,
  normalizeAcmeCertificateAuthority,
  type AcmeCertificateAuthority,
} from "../acme-certificate-authority";
import {
  normalizeDomainList,
  normalizeStringRecord,
  normalizeTimestamp,
} from "./app-config";
import type { AcmeClientSettings, AcmeSettings } from "./types";

export class AcmeSettingsStore {
  private readonly legacySettingsKey = "fn_knock:acme:settings";
  private readonly clientSettingsKey = "fn_knock:acme:client-settings";

  constructor(private readonly redis: Redis) {}

  async readLegacySettings(): Promise<AcmeSettings | null> {
    const raw = await this.redis.get(this.legacySettingsKey);
    if (!raw) return null;
    try {
      const obj = JSON.parse(raw);
      if (!obj || typeof obj !== "object") return null;
      if (
        !Array.isArray(obj.domains) ||
        typeof obj.dnsType !== "string" ||
        typeof obj.credentials !== "object"
      ) {
        return null;
      }
      return {
        domains: normalizeDomainList(obj.domains),
        dnsType: String(obj.dnsType || "").trim(),
        credentials: normalizeStringRecord(obj.credentials),
        updatedAt:
          normalizeTimestamp(obj.updatedAt) || new Date().toISOString(),
      };
    } catch {
      return null;
    }
  }

  async saveLegacySettings(value: AcmeSettings): Promise<void> {
    await this.redis.set(this.legacySettingsKey, JSON.stringify(value));
  }

  async deleteLegacySettings(): Promise<void> {
    await this.redis.del(this.legacySettingsKey);
  }

  async saveClientSettings(
    value: Pick<AcmeClientSettings, "certificateAuthority">,
  ): Promise<AcmeClientSettings> {
    const next: AcmeClientSettings = {
      certificateAuthority: normalizeAcmeCertificateAuthority(
        value.certificateAuthority,
      ),
      updatedAt: new Date().toISOString(),
    };
    await this.redis.set(this.clientSettingsKey, JSON.stringify(next));
    return next;
  }

  async getClientSettings(): Promise<AcmeClientSettings | null> {
    const raw = await this.redis.get(this.clientSettingsKey);
    if (!raw) return null;
    try {
      const obj = JSON.parse(raw);
      if (!obj || typeof obj !== "object") return null;
      return {
        certificateAuthority: normalizeAcmeCertificateAuthority(
          typeof obj.certificateAuthority === "string"
            ? obj.certificateAuthority
            : undefined,
        ),
        updatedAt:
          typeof obj.updatedAt === "string"
            ? obj.updatedAt
            : new Date().toISOString(),
      };
    } catch {
      return null;
    }
  }

  async ensureClientSettings(
    fallbackCertificateAuthority: AcmeCertificateAuthority = DEFAULT_ACME_CERTIFICATE_AUTHORITY,
  ): Promise<AcmeClientSettings> {
    const existing = await this.getClientSettings();
    if (existing) return existing;
    return this.saveClientSettings({
      certificateAuthority: fallbackCertificateAuthority,
    });
  }
}
