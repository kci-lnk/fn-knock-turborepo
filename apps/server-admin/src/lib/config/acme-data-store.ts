import { createHash, randomBytes } from "node:crypto";
import type Redis from "ioredis";
import {
  normalizeAcmeApplication,
  normalizeAcmeIssuedCertificate,
} from "./app-config";
import type { AcmeApplication, AcmeIssuedCertificate } from "./types";

export class AcmeDataStore {
  private acmeApplicationsKey = "fn_knock:acme:applications";
  private acmeIssuedCertificatesKey = "fn_knock:acme:issued-certificates";

  constructor(private readonly redis: Redis) {}

  buildApplicationId(seed?: string): string {
    const normalizedSeed = seed?.trim().toLowerCase();
    if (normalizedSeed) {
      return `acme_app_${createHash("sha256")
        .update(normalizedSeed)
        .digest("hex")
        .slice(0, 16)}`;
    }

    return `acme_app_${randomBytes(8).toString("hex")}`;
  }

  async readApplications(): Promise<AcmeApplication[]> {
    const raw = await this.redis.get(this.acmeApplicationsKey);
    if (!raw) return [];
    try {
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed
        .map((item) => normalizeAcmeApplication(item))
        .filter((item): item is AcmeApplication => item !== null);
    } catch {
      return [];
    }
  }

  async writeApplications(applications: AcmeApplication[]): Promise<void> {
    await this.redis.set(
      this.acmeApplicationsKey,
      JSON.stringify(applications),
    );
  }

  async readIssuedCertificates(): Promise<AcmeIssuedCertificate[]> {
    const raw = await this.redis.get(this.acmeIssuedCertificatesKey);
    if (!raw) return [];
    try {
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed
        .map((item) => normalizeAcmeIssuedCertificate(item))
        .filter((item): item is AcmeIssuedCertificate => item !== null);
    } catch {
      return [];
    }
  }

  async writeIssuedCertificates(
    issuedCertificates: AcmeIssuedCertificate[],
  ): Promise<void> {
    await this.redis.set(
      this.acmeIssuedCertificatesKey,
      JSON.stringify(issuedCertificates),
    );
  }
}
