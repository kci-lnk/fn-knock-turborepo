import type { AcmeCertificateStore } from "./acme-certificate-store";
import type { AcmeDataStore } from "./acme-data-store";
import {
  hasSameNormalizedDomainSet,
  mirrorActiveSSLCertificate,
} from "./app-config";
import { redisT } from "./messages";
import type { SSLConfigStore } from "./ssl-store";
import type {
  AcmeApplication,
  AcmeIssuedCertificate,
  AppConfig,
  SSLManagedCertificate,
} from "./types";

type ConfigAccess = {
  getConfig: () => Promise<AppConfig>;
  saveConfig: (config: AppConfig) => Promise<void>;
};

type AcmeLookupAccess = {
  getApplication: (id: string) => Promise<AcmeApplication | null>;
  getApplicationByPrimaryDomain: (
    primaryDomain: string,
  ) => Promise<AcmeApplication | null>;
  getIssuedCertificate: (
    applicationId: string,
  ) => Promise<AcmeIssuedCertificate | null>;
  linkIssuedCertificateToLibrary: (
    applicationId: string,
    libraryCertificateId?: string | null,
  ) => Promise<AcmeIssuedCertificate | null>;
};

export class AcmeLibraryService {
  constructor(
    private readonly args: {
      access: ConfigAccess;
      acmeCertificateStore: AcmeCertificateStore;
      acmeDataStore: AcmeDataStore;
      lookups: AcmeLookupAccess;
      sslStore: SSLConfigStore;
    },
  ) {}

  isIssuedCertificateCompatible(
    application:
      | Pick<AcmeApplication, "domains" | "primaryDomain">
      | null
      | undefined,
    issuedCertificate:
      | Pick<AcmeIssuedCertificate, "primaryDomain" | "certInfo">
      | null
      | undefined,
  ): boolean {
    if (!application || !issuedCertificate) return false;
    if (issuedCertificate.primaryDomain !== application.primaryDomain) {
      return false;
    }
    return hasSameNormalizedDomainSet(
      application.domains,
      issuedCertificate.certInfo.dnsNames,
    );
  }

  async getUsableIssuedCertificate(
    applicationId: string,
  ): Promise<AcmeIssuedCertificate | null> {
    const [application, issuedCertificate] = await Promise.all([
      this.args.lookups.getApplication(applicationId),
      this.args.lookups.getIssuedCertificate(applicationId),
    ]);
    if (!this.isIssuedCertificateCompatible(application, issuedCertificate)) {
      return null;
    }
    return issuedCertificate;
  }

  async saveCertificateToLibrary(
    domain: string,
    opts?: {
      id?: string;
      label?: string;
      activate?: boolean;
    },
  ): Promise<SSLManagedCertificate> {
    const normalizedDomain = domain.trim().toLowerCase();
    if (!normalizedDomain) {
      throw new Error(redisT("acme.domainRequired"));
    }

    const application =
      await this.args.lookups.getApplicationByPrimaryDomain(normalizedDomain);
    if (application) {
      return this.saveCertificateToLibraryByApplication(application.id, opts);
    }

    const pair = await this.args.acmeCertificateStore.get(normalizedDomain);
    if (!pair) {
      throw new Error(redisT("ssl.certNotFound"));
    }

    const validation = this.args.sslStore.validateSSLCert(pair.cert, pair.key);
    if (!validation.valid) {
      throw new Error(validation.error || redisT("ssl.certOrKeyInvalid"));
    }

    return this.args.sslStore.saveSSLCertificate({
      id: opts?.id,
      label: opts?.label || normalizedDomain,
      source: "acme",
      primary_domain: normalizedDomain,
      cert: pair.cert,
      key: pair.key,
      activate: opts?.activate === true,
      matchBy: {
        source: "acme",
        primary_domain: normalizedDomain,
      },
    });
  }

  async saveCertificateToLibraryByApplication(
    applicationId: string,
    opts?: {
      id?: string;
      label?: string;
      activate?: boolean;
    },
  ): Promise<SSLManagedCertificate> {
    const [application, issuedCertificate] = await Promise.all([
      this.args.lookups.getApplication(applicationId),
      this.args.lookups.getIssuedCertificate(applicationId),
    ]);
    if (!application) {
      throw new Error(redisT("acme.applicationNotFound"));
    }
    if (!this.isIssuedCertificateCompatible(application, issuedCertificate)) {
      throw new Error(redisT("acme.noMatchingIssuedCertificate"));
    }
    if (!issuedCertificate) {
      throw new Error(redisT("ssl.certNotFound"));
    }

    const validation = this.args.sslStore.validateSSLCert(
      issuedCertificate.cert,
      issuedCertificate.key,
    );
    if (!validation.valid) {
      throw new Error(validation.error || redisT("ssl.certOrKeyInvalid"));
    }

    const saved = await this.args.sslStore.saveSSLCertificate({
      id: opts?.id || issuedCertificate.libraryCertificateId,
      label: opts?.label || issuedCertificate.primaryDomain,
      source: "acme",
      primary_domain: issuedCertificate.primaryDomain,
      source_ref_id: applicationId,
      cert: issuedCertificate.cert,
      key: issuedCertificate.key,
      activate: opts?.activate === true,
      matchBy: {
        source: "acme",
        source_ref_id: applicationId,
      },
    });

    await this.args.lookups.linkIssuedCertificateToLibrary(
      applicationId,
      saved.id,
    );
    return saved;
  }

  async reconcileLibraryLinks(
    applications: AcmeApplication[],
    issuedCertificates: AcmeIssuedCertificate[],
  ): Promise<AcmeIssuedCertificate[]> {
    if (!applications.length) return issuedCertificates;

    const config = await this.args.access.getConfig();
    const certificates = [...(config.ssl.certificates || [])];
    const nextIssuedCertificates = issuedCertificates.map((item) => ({
      ...item,
    }));
    let configChanged = false;
    let issuedChanged = false;

    for (const application of applications) {
      const certificateIndex = certificates.findIndex((certificate) => {
        if (certificate.source !== "acme") return false;
        if (certificate.source_ref_id === application.id) return true;
        return (
          !certificate.source_ref_id &&
          certificate.primary_domain === application.primaryDomain
        );
      });

      if (certificateIndex === -1) continue;

      const libraryCertificate = certificates[certificateIndex]!;
      if (libraryCertificate.source_ref_id !== application.id) {
        certificates[certificateIndex] = {
          ...libraryCertificate,
          source_ref_id: application.id,
        };
        configChanged = true;
      }

      const issuedIndex = nextIssuedCertificates.findIndex(
        (item) => item.applicationId === application.id,
      );
      if (issuedIndex === -1) continue;

      const issuedCertificate = nextIssuedCertificates[issuedIndex]!;
      if (
        issuedCertificate.libraryCertificateId !== libraryCertificate.id ||
        !issuedCertificate.libraryLinkedAt
      ) {
        nextIssuedCertificates[issuedIndex] = {
          ...issuedCertificate,
          libraryCertificateId: libraryCertificate.id,
          libraryLinkedAt:
            issuedCertificate.libraryLinkedAt || libraryCertificate.updated_at,
        };
        issuedChanged = true;
      }
    }

    for (const [index, issuedCertificate] of nextIssuedCertificates.entries()) {
      if (!issuedCertificate.libraryCertificateId) continue;
      const stillExists = certificates.some(
        (certificate) =>
          certificate.source === "acme" &&
          certificate.id === issuedCertificate.libraryCertificateId,
      );
      if (stillExists) continue;
      nextIssuedCertificates[index] = {
        ...issuedCertificate,
        libraryCertificateId: undefined,
        libraryLinkedAt: undefined,
      };
      issuedChanged = true;
    }

    if (configChanged) {
      config.ssl = {
        ...config.ssl,
        certificates,
      };
      config.ssl = mirrorActiveSSLCertificate(
        config.ssl,
        config.ssl.active_cert_id,
      );
      await this.args.access.saveConfig(config);
    }

    if (issuedChanged) {
      await this.args.acmeDataStore.writeIssuedCertificates(
        nextIssuedCertificates,
      );
    }

    return nextIssuedCertificates;
  }
}
