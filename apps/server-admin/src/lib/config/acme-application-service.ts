import type Redis from "ioredis";

import { cleanupAcmeApplicationArtifacts } from "./acme-cleanup";
import type { AcmeCertificateStore } from "./acme-certificate-store";
import type { AcmeDataStore } from "./acme-data-store";
import type { AcmeLibraryService } from "./acme-library-service";
import type { AcmeSettingsStore } from "./acme-settings-store";
import {
  hasSameNormalizedDomainSet,
  normalizeDomainList,
  normalizeStringRecord,
} from "./app-config";
import { redisT } from "./messages";
import { normalizeOptionalString } from "./normalizers";
import type { SSLConfigStore } from "./ssl-store";
import type {
  AcmeApplication,
  AcmeApplicationDeleteResult,
  AcmeApplicationSaveResult,
  AcmeIssuedCertificate,
  AcmeJob,
  AcmeSettings,
  SSLCertInfo,
} from "./types";

const ACME_MIGRATION_VERSION_KEY = "fn_knock:acme:migration:v1";

export class AcmeApplicationService {
  constructor(
    private readonly args: {
      acmeCertificateStore: AcmeCertificateStore;
      acmeDataStore: AcmeDataStore;
      acmeLibraryService: AcmeLibraryService;
      acmeSettingsStore: AcmeSettingsStore;
      parseCertInfo: (certPem: string) => SSLCertInfo | null;
      redis: Redis;
      sslStore: SSLConfigStore;
    },
  ) {}

  async ensureDataMigrated(): Promise<void> {
    const migrationVersion = await this.args.redis.get(
      ACME_MIGRATION_VERSION_KEY,
    );
    const existingApplications =
      await this.args.acmeDataStore.readApplications();

    if (existingApplications.length > 0) {
      const issuedCertificates =
        await this.args.acmeDataStore.readIssuedCertificates();
      await this.args.acmeLibraryService.reconcileLibraryLinks(
        existingApplications,
        issuedCertificates,
      );
      if (migrationVersion !== "1") {
        await this.args.redis.set(ACME_MIGRATION_VERSION_KEY, "1");
      }
      return;
    }

    const legacySettings =
      await this.args.acmeSettingsStore.readLegacySettings();
    if (!legacySettings?.domains?.length) {
      if (migrationVersion !== "1") {
        await this.args.redis.set(ACME_MIGRATION_VERSION_KEY, "1");
      }
      return;
    }

    const now = new Date().toISOString();
    const primaryDomain = legacySettings.domains[0]!;
    const application: AcmeApplication = {
      id: this.args.acmeDataStore.buildApplicationId(primaryDomain),
      domains: legacySettings.domains,
      primaryDomain,
      dnsType: legacySettings.dnsType,
      credentials: { ...legacySettings.credentials },
      renewEnabled: true,
      createdAt: legacySettings.updatedAt || now,
      updatedAt: legacySettings.updatedAt || now,
      latestJobStatus: "idle",
    };

    const issuedCertificates: AcmeIssuedCertificate[] = [];
    const pair = await this.args.acmeCertificateStore.get(primaryDomain);
    if (pair) {
      const certInfo = this.args.parseCertInfo(pair.cert);
      if (certInfo) {
        issuedCertificates.push({
          applicationId: application.id,
          primaryDomain,
          cert: pair.cert,
          key: pair.key,
          certInfo,
          createdAt: now,
          updatedAt: now,
        });
      }
    }

    await this.args.acmeDataStore.writeApplications([application]);
    await this.args.acmeDataStore.writeIssuedCertificates(issuedCertificates);
    await this.args.acmeLibraryService.reconcileLibraryLinks(
      [application],
      issuedCertificates,
    );
    await this.args.redis.set(ACME_MIGRATION_VERSION_KEY, "1");
  }

  async listApplications(): Promise<AcmeApplication[]> {
    await this.ensureDataMigrated();
    const applications = await this.args.acmeDataStore.readApplications();
    return applications.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  }

  async getApplication(id: string): Promise<AcmeApplication | null> {
    await this.ensureDataMigrated();
    const applications = await this.args.acmeDataStore.readApplications();
    return applications.find((item) => item.id === id) || null;
  }

  async getApplicationByPrimaryDomain(
    primaryDomain: string,
  ): Promise<AcmeApplication | null> {
    await this.ensureDataMigrated();
    const normalizedPrimaryDomain = primaryDomain.trim().toLowerCase();
    if (!normalizedPrimaryDomain) return null;
    const applications = await this.args.acmeDataStore.readApplications();
    return (
      applications.find(
        (item) => item.primaryDomain === normalizedPrimaryDomain,
      ) || null
    );
  }

  async saveApplication(input: {
    id?: string;
    name?: string;
    domains: string[];
    dnsType: string;
    credentials: Record<string, string>;
    renewEnabled?: boolean;
  }): Promise<AcmeApplication> {
    const result = await this.saveApplicationWithEffects(input);
    return result.application;
  }

  async deleteApplication(
    id: string,
  ): Promise<AcmeApplicationDeleteResult | null> {
    await this.ensureDataMigrated();
    const applications = await this.args.acmeDataStore.readApplications();
    const existing = applications.find((item) => item.id === id) || null;
    if (!existing) return null;

    const nextApplications = applications.filter((item) => item.id !== id);
    await this.args.acmeDataStore.writeApplications(nextApplications);
    if (nextApplications.length === 0) {
      await this.args.acmeSettingsStore.deleteLegacySettings();
    }

    const deletedIssuedCertificate = await this.deleteIssuedCertificate(id);
    const cleanup = await cleanupAcmeApplicationArtifacts({
      applicationId: id,
      deleteAcmeCert: (domain) => this.args.acmeCertificateStore.delete(domain),
      deletedIssuedCertificate,
      deleteSSLCertificatesBySource: (source, primaryDomain) =>
        this.args.sslStore.deleteSSLCertificatesBySource(
          source,
          primaryDomain,
        ),
      deleteSSLCertificatesBySourceRef: (source, sourceRefId) =>
        this.args.sslStore.deleteSSLCertificatesBySourceRef(
          source,
          sourceRefId,
        ),
      primaryDomain: existing.primaryDomain,
    });

    return {
      application: existing,
      deletedIssuedCertificate,
      removedLibraryCertificates: cleanup.removedLibraryCertificates,
      removedActiveLibraryCertificate:
        cleanup.removedActiveLibraryCertificate,
      removedDomains: cleanup.removedDomains,
    };
  }

  async saveApplicationWithEffects(input: {
    id?: string;
    name?: string;
    domains: string[];
    dnsType: string;
    credentials: Record<string, string>;
    renewEnabled?: boolean;
  }): Promise<AcmeApplicationSaveResult> {
    await this.ensureDataMigrated();
    const applications = await this.args.acmeDataStore.readApplications();
    const normalizedDomains = normalizeDomainList(input.domains);
    const primaryDomain = normalizedDomains[0] || "";
    const dnsType = input.dnsType.trim();

    if (!normalizedDomains.length) {
      throw new Error(redisT("acme.domainsRequired"));
    }
    if (!dnsType) {
      throw new Error(redisT("acme.dnsProviderRequired"));
    }

    const existing = input.id
      ? applications.find((item) => item.id === input.id) || null
      : null;
    const duplicated = applications.find(
      (item) =>
        item.primaryDomain === primaryDomain && item.id !== existing?.id,
    );
    if (duplicated) {
      throw new Error(
        redisT("acme.primaryDomainDuplicated", { primaryDomain }),
      );
    }

    const now = new Date().toISOString();
    const next: AcmeApplication = {
      id: existing?.id || this.args.acmeDataStore.buildApplicationId(),
      name:
        input.name !== undefined
          ? normalizeOptionalString(input.name)
          : existing?.name,
      domains: normalizedDomains,
      primaryDomain,
      dnsType,
      credentials: normalizeStringRecord(input.credentials),
      renewEnabled: input.renewEnabled ?? existing?.renewEnabled ?? true,
      createdAt: existing?.createdAt || now,
      updatedAt: now,
      latestJobId: existing?.latestJobId,
      latestJobStatus: existing?.latestJobStatus || "idle",
      latestJobTrigger: existing?.latestJobTrigger,
      latestJobAt: existing?.latestJobAt,
      lastError: existing?.lastError,
    };

    const nextApplications = applications.filter((item) => item.id !== next.id);
    nextApplications.unshift(next);
    await this.args.acmeDataStore.writeApplications(nextApplications);

    const domainChanged =
      !!existing &&
      (!hasSameNormalizedDomainSet(existing.domains, next.domains) ||
        existing.primaryDomain !== next.primaryDomain);

    if (!domainChanged) {
      return {
        application: next,
        certificateInvalidated: false,
        deletedIssuedCertificate: null,
        removedLibraryCertificates: [],
        removedActiveLibraryCertificate: false,
        removedDomains: [],
      };
    }

    const deletedIssuedCertificate = await this.deleteIssuedCertificate(
      next.id,
    );
    const cleanup = await cleanupAcmeApplicationArtifacts({
      applicationId: next.id,
      deleteAcmeCert: (domain) => this.args.acmeCertificateStore.delete(domain),
      deletedIssuedCertificate,
      deleteSSLCertificatesBySource: (source, primaryDomain) =>
        this.args.sslStore.deleteSSLCertificatesBySource(
          source,
          primaryDomain,
        ),
      deleteSSLCertificatesBySourceRef: (source, sourceRefId) =>
        this.args.sslStore.deleteSSLCertificatesBySourceRef(
          source,
          sourceRefId,
        ),
      primaryDomain: existing.primaryDomain,
    });

    return {
      application: next,
      certificateInvalidated: true,
      deletedIssuedCertificate,
      removedLibraryCertificates: cleanup.removedLibraryCertificates,
      removedActiveLibraryCertificate:
        cleanup.removedActiveLibraryCertificate,
      removedDomains: cleanup.removedDomains,
    };
  }

  async updateApplicationJobState(
    applicationId: string,
    job: Pick<
      AcmeJob,
      | "id"
      | "status"
      | "trigger"
      | "createdAt"
      | "startedAt"
      | "finishedAt"
      | "message"
    >,
  ): Promise<AcmeApplication | null> {
    await this.ensureDataMigrated();
    const applications = await this.args.acmeDataStore.readApplications();
    const index = applications.findIndex((item) => item.id === applicationId);
    if (index === -1) return null;

    const existing = applications[index]!;
    const latestJobAt =
      job.finishedAt ||
      job.startedAt ||
      job.createdAt ||
      new Date().toISOString();

    const next: AcmeApplication = {
      ...existing,
      latestJobId: job.id,
      latestJobStatus: job.status,
      latestJobTrigger: job.trigger,
      latestJobAt,
      lastError:
        job.status === "failed"
          ? normalizeOptionalString(job.message)
          : undefined,
    };

    applications[index] = next;
    await this.args.acmeDataStore.writeApplications(applications);
    return next;
  }

  async listIssuedCertificates(): Promise<AcmeIssuedCertificate[]> {
    await this.ensureDataMigrated();
    return this.args.acmeDataStore.readIssuedCertificates();
  }

  async getIssuedCertificate(
    applicationId: string,
  ): Promise<AcmeIssuedCertificate | null> {
    await this.ensureDataMigrated();
    const issuedCertificates =
      await this.args.acmeDataStore.readIssuedCertificates();
    return (
      issuedCertificates.find((item) => item.applicationId === applicationId) ||
      null
    );
  }

  async getIssuedCertificateByPrimaryDomain(
    primaryDomain: string,
  ): Promise<AcmeIssuedCertificate | null> {
    await this.ensureDataMigrated();
    const normalizedPrimaryDomain = primaryDomain.trim().toLowerCase();
    if (!normalizedPrimaryDomain) return null;
    const issuedCertificates =
      await this.args.acmeDataStore.readIssuedCertificates();
    return (
      issuedCertificates.find(
        (item) => item.primaryDomain === normalizedPrimaryDomain,
      ) || null
    );
  }

  async saveIssuedCertificate(input: {
    applicationId: string;
    primaryDomain: string;
    cert: string;
    key: string;
    certInfo: SSLCertInfo;
    libraryCertificateId?: string;
  }): Promise<AcmeIssuedCertificate> {
    await this.ensureDataMigrated();
    const issuedCertificates =
      await this.args.acmeDataStore.readIssuedCertificates();
    const existing =
      issuedCertificates.find(
        (item) => item.applicationId === input.applicationId,
      ) || null;
    const now = new Date().toISOString();

    const next: AcmeIssuedCertificate = {
      applicationId: input.applicationId.trim(),
      primaryDomain: input.primaryDomain.trim().toLowerCase(),
      cert: input.cert.trim(),
      key: input.key.trim(),
      certInfo: input.certInfo,
      createdAt: existing?.createdAt || now,
      updatedAt: now,
      libraryCertificateId:
        normalizeOptionalString(input.libraryCertificateId) ||
        existing?.libraryCertificateId,
      libraryLinkedAt: normalizeOptionalString(input.libraryCertificateId)
        ? now
        : existing?.libraryLinkedAt,
    };

    const nextIssuedCertificates = issuedCertificates.filter(
      (item) => item.applicationId !== next.applicationId,
    );
    nextIssuedCertificates.unshift(next);
    await this.args.acmeDataStore.writeIssuedCertificates(
      nextIssuedCertificates,
    );
    await this.args.acmeCertificateStore.save(
      next.primaryDomain,
      next.cert,
      next.key,
    );
    return next;
  }

  async linkIssuedCertificateToLibrary(
    applicationId: string,
    libraryCertificateId?: string | null,
  ): Promise<AcmeIssuedCertificate | null> {
    await this.ensureDataMigrated();
    const issuedCertificates =
      await this.args.acmeDataStore.readIssuedCertificates();
    const index = issuedCertificates.findIndex(
      (item) => item.applicationId === applicationId,
    );
    if (index === -1) return null;

    const existing = issuedCertificates[index]!;
    issuedCertificates[index] = {
      ...existing,
      updatedAt: new Date().toISOString(),
      libraryCertificateId:
        normalizeOptionalString(libraryCertificateId) || undefined,
      libraryLinkedAt: libraryCertificateId
        ? new Date().toISOString()
        : undefined,
    };
    await this.args.acmeDataStore.writeIssuedCertificates(issuedCertificates);
    return issuedCertificates[index]!;
  }

  async deleteIssuedCertificate(
    applicationId: string,
  ): Promise<AcmeIssuedCertificate | null> {
    await this.ensureDataMigrated();
    const issuedCertificates =
      await this.args.acmeDataStore.readIssuedCertificates();
    const existing =
      issuedCertificates.find((item) => item.applicationId === applicationId) ||
      null;
    if (!existing) return null;

    const nextIssuedCertificates = issuedCertificates.filter(
      (item) => item.applicationId !== applicationId,
    );
    await this.args.acmeDataStore.writeIssuedCertificates(
      nextIssuedCertificates,
    );
    await this.args.acmeCertificateStore.delete(existing.primaryDomain);
    return existing;
  }

  async saveIssuedCertFromFS(
    applicationId: string,
    primaryDomain: string,
    opts?: {
      forceInstall?: boolean;
      onLog?: (line: string) => Promise<void> | void;
    },
  ): Promise<boolean> {
    const saved = await this.args.acmeCertificateStore.saveFromFS(
      primaryDomain,
      opts,
    );
    if (!saved) return false;

    const pair = await this.args.acmeCertificateStore.get(primaryDomain);
    if (!pair) return false;

    const certInfo = this.args.parseCertInfo(pair.cert);
    if (!certInfo) return false;

    const existing = await this.getIssuedCertificate(applicationId);
    await this.saveIssuedCertificate({
      applicationId,
      primaryDomain,
      cert: pair.cert,
      key: pair.key,
      certInfo,
      libraryCertificateId: existing?.libraryCertificateId,
    });
    return true;
  }

  async saveSettings(
    value: Omit<AcmeSettings, "updatedAt">,
  ): Promise<AcmeSettings> {
    await this.ensureDataMigrated();
    const applications = await this.args.acmeDataStore.readApplications();
    const normalizedDomains = normalizeDomainList(value.domains);
    const primaryDomain = normalizedDomains[0] || "";
    const targetApplication =
      applications.find((item) => item.primaryDomain === primaryDomain) ||
      (applications.length === 1 ? applications[0]! : null);

    if (!targetApplication && applications.length > 1) {
      throw new Error(redisT("acme.multipleApplicationsUseNewApi"));
    }

    const savedApplication = await this.saveApplication({
      id: targetApplication?.id,
      domains: normalizedDomains,
      dnsType: value.dnsType,
      credentials: value.credentials,
      renewEnabled: targetApplication?.renewEnabled ?? true,
      name: targetApplication?.name,
    });

    const next: AcmeSettings = {
      domains: savedApplication.domains,
      dnsType: savedApplication.dnsType,
      credentials: savedApplication.credentials,
      updatedAt: savedApplication.updatedAt,
    };
    await this.args.acmeSettingsStore.saveLegacySettings(next);
    return next;
  }

  async getSettings(): Promise<AcmeSettings | null> {
    await this.ensureDataMigrated();
    const applications = await this.args.acmeDataStore.readApplications();
    const application = applications[0];
    if (application) {
      return {
        domains: application.domains,
        dnsType: application.dnsType,
        credentials: application.credentials,
        updatedAt: application.updatedAt,
      };
    }
    return this.args.acmeSettingsStore.readLegacySettings();
  }

}
