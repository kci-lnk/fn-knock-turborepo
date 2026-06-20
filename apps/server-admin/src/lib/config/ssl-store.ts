import { X509Certificate, createPrivateKey } from "node:crypto";
import {
  findMatchingSSLCertificate,
  mirrorActiveSSLCertificate,
  normalizeManagedSSLCertificate,
  normalizeSSLConfig,
} from "./app-config";
import { redisT } from "./messages";
import type {
  AppConfig,
  SSLCertInfo,
  SSLManagedCertificate,
  SSLCertificateSource,
  SSLStatus,
} from "./types";

export interface SSLConfigStoreDependencies {
  getConfig: () => Promise<AppConfig>;
  saveConfig: (config: AppConfig) => Promise<void>;
}

export interface SaveSSLCertificateInput {
  id?: string;
  label?: string;
  source?: SSLCertificateSource;
  primary_domain?: string;
  source_ref_id?: string;
  cert: string;
  key: string;
  activate?: boolean;
  matchBy?: {
    source?: SSLCertificateSource;
    primary_domain?: string;
    source_ref_id?: string;
    cert?: string;
    key?: string;
  };
}

export class SSLConfigStore {
  constructor(private readonly deps: SSLConfigStoreDependencies) {}

  parseCertInfo(certPem: string): SSLCertInfo | null {
    try {
      const x509 = new X509Certificate(certPem);
      const sanStr = x509.subjectAltName || "";
      const dnsNames: string[] = [];
      sanStr
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean)
        .forEach((entry) => {
          const idx = entry.indexOf(":");
          if (idx <= 0) return;
          const label = entry.slice(0, idx).trim().toLowerCase();
          const value = entry.slice(idx + 1).trim();
          if (label === "dns" || label === "ip" || label === "ip address") {
            dnsNames.push(value);
          }
        });
      const subjectCommonName =
        x509.subject
          .split("\n")
          .map((entry) => entry.trim())
          .find((entry) => /^CN\s*=/.test(entry))
          ?.replace(/^CN\s*=\s*/i, "")
          .trim() || "";
      if (
        subjectCommonName &&
        !dnsNames.some(
          (entry) => entry.toLowerCase() === subjectCommonName.toLowerCase(),
        )
      ) {
        dnsNames.push(subjectCommonName);
      }

      return {
        issuer: x509.issuer,
        subject: x509.subject,
        validFrom: x509.validFrom,
        validTo: x509.validTo,
        dnsNames,
        serialNumber: x509.serialNumber,
      };
    } catch (e) {
      console.error("Failed to parse X.509 certificate:", e);
      return null;
    }
  }

  async getSSLStatus(): Promise<SSLStatus> {
    const config = await this.deps.getConfig();
    const ssl = normalizeSSLConfig(config.ssl);
    const activeCertId = ssl.active_cert_id?.trim() || "";
    const certificates = (ssl.certificates || []).map((item) => {
      const certInfo = this.parseCertInfo(item.cert);
      return {
        id: item.id,
        label: item.label,
        source: item.source,
        primary_domain: item.primary_domain,
        source_ref_id: item.source_ref_id,
        created_at: item.created_at,
        updated_at: item.updated_at,
        certInfo: certInfo || undefined,
        is_active: item.id === activeCertId,
      };
    });
    const activeCertificate =
      certificates.find((item) => item.is_active) || null;
    const certInfo = activeCertificate?.certInfo;

    return {
      enabled: !!activeCertificate,
      activeCertId: activeCertificate?.id,
      deploymentMode: ssl.deployment_mode || "single_active",
      certInfo: certInfo || undefined,
      certificates,
    };
  }

  validateSSLCert(
    cert: string,
    key: string,
  ): { valid: boolean; error?: string } {
    try {
      new X509Certificate(cert);
    } catch (e: any) {
      return {
        valid: false,
        error: redisT("ssl.certFormatInvalid", { message: e.message }),
      };
    }
    try {
      createPrivateKey(key);
    } catch (e: any) {
      return {
        valid: false,
        error: redisT("ssl.keyFormatInvalid", { message: e.message }),
      };
    }
    try {
      const x509 = new X509Certificate(cert);
      const privateKey = createPrivateKey(key);
      if (!x509.checkPrivateKey(privateKey)) {
        return { valid: false, error: redisT("ssl.certKeyMismatch") };
      }
    } catch (e: any) {
      return {
        valid: false,
        error: redisT("ssl.certKeyCheckFailed", { message: e.message }),
      };
    }
    return { valid: true };
  }

  async clearSSL(): Promise<void> {
    const config = await this.deps.getConfig();
    config.ssl = mirrorActiveSSLCertificate(config.ssl, null);
    await this.deps.saveConfig(config);
  }

  async clearSSLCertificateLibrary(): Promise<number> {
    const config = await this.deps.getConfig();
    const removedCount = config.ssl.certificates?.length || 0;
    config.ssl = {
      ...config.ssl,
      certificates: [],
    };
    config.ssl = mirrorActiveSSLCertificate(config.ssl, null);
    await this.deps.saveConfig(config);
    return removedCount;
  }

  async getSSLCertificate(id: string): Promise<SSLManagedCertificate | null> {
    const config = await this.deps.getConfig();
    return (
      config.ssl.certificates?.find((certificate) => certificate.id === id) ||
      null
    );
  }

  async getActiveSSLCertificate(): Promise<SSLManagedCertificate | null> {
    const config = await this.deps.getConfig();
    const activeId = config.ssl.active_cert_id?.trim();
    if (!activeId) return null;
    return (
      config.ssl.certificates?.find(
        (certificate) => certificate.id === activeId,
      ) || null
    );
  }

  async saveSSLCertificate(
    input: SaveSSLCertificateInput,
  ): Promise<SSLManagedCertificate> {
    const config = await this.deps.getConfig();
    const ssl = normalizeSSLConfig(config.ssl);
    const certificates = [...(ssl.certificates || [])];
    const now = new Date().toISOString();

    let existing =
      (input.id
        ? certificates.find((certificate) => certificate.id === input.id)
        : undefined) || null;

    if (
      !existing &&
      input.matchBy?.source &&
      input.matchBy?.source_ref_id?.trim()
    ) {
      existing =
        certificates.find(
          (certificate) =>
            certificate.source === input.matchBy?.source &&
            certificate.source_ref_id === input.matchBy?.source_ref_id?.trim(),
        ) || null;
    }

    if (
      !existing &&
      input.matchBy?.source &&
      input.matchBy?.primary_domain?.trim()
    ) {
      existing =
        certificates.find(
          (certificate) =>
            certificate.source === input.matchBy?.source &&
            certificate.primary_domain ===
              input.matchBy?.primary_domain?.trim().toLowerCase(),
        ) || null;
    }

    if (!existing && input.matchBy?.cert && input.matchBy?.key) {
      existing = findMatchingSSLCertificate(
        certificates,
        input.matchBy.cert.trim(),
        input.matchBy.key.trim(),
      );
    }

    const nextRecord = normalizeManagedSSLCertificate({
      id: existing?.id || input.id,
      label: input.label || existing?.label,
      source: input.source || existing?.source || "manual",
      primary_domain: input.primary_domain || existing?.primary_domain,
      source_ref_id: input.source_ref_id || existing?.source_ref_id,
      cert: input.cert,
      key: input.key,
      created_at: existing?.created_at || now,
      updated_at: now,
    });

    if (!nextRecord) {
      throw new Error(redisT("ssl.certContentRequired"));
    }

    const nextCertificates = certificates.filter(
      (certificate) => certificate.id !== nextRecord.id,
    );
    nextCertificates.unshift(nextRecord);

    config.ssl = {
      ...ssl,
      certificates: nextCertificates,
    };
    config.ssl = mirrorActiveSSLCertificate(
      config.ssl,
      input.activate === true ? nextRecord.id : ssl.active_cert_id,
    );
    await this.deps.saveConfig(config);
    return nextRecord;
  }

  async getSSLCertificateBySourceRef(
    source: SSLCertificateSource,
    sourceRefId: string,
  ): Promise<SSLManagedCertificate | null> {
    const normalizedSourceRefId = sourceRefId.trim();
    if (!normalizedSourceRefId) return null;
    const config = await this.deps.getConfig();
    return (
      config.ssl.certificates?.find(
        (certificate) =>
          certificate.source === source &&
          certificate.source_ref_id === normalizedSourceRefId,
      ) || null
    );
  }

  async activateSSLCertificate(
    id: string | null | undefined,
  ): Promise<SSLManagedCertificate | null> {
    const config = await this.deps.getConfig();
    const normalizedId = typeof id === "string" ? id.trim() : "";
    const active = normalizedId
      ? config.ssl.certificates?.find(
          (certificate) => certificate.id === normalizedId,
        )
      : null;

    config.ssl = mirrorActiveSSLCertificate(config.ssl, active?.id || null);
    await this.deps.saveConfig(config);
    return active || null;
  }

  async deleteSSLCertificate(id: string): Promise<{
    removed: SSLManagedCertificate | null;
    removedActive: boolean;
  }> {
    const config = await this.deps.getConfig();
    const certificates = [...(config.ssl.certificates || [])];
    const removed =
      certificates.find((certificate) => certificate.id === id) || null;
    if (!removed) {
      return { removed: null, removedActive: false };
    }

    const removedActive = config.ssl.active_cert_id === removed.id;
    config.ssl = {
      ...config.ssl,
      certificates: certificates.filter((certificate) => certificate.id !== id),
    };
    config.ssl = mirrorActiveSSLCertificate(
      config.ssl,
      removedActive ? null : config.ssl.active_cert_id,
    );
    await this.deps.saveConfig(config);
    return { removed, removedActive };
  }

  async deleteSSLCertificatesBySource(
    source: SSLCertificateSource,
    primaryDomain?: string,
  ): Promise<{
    removed: SSLManagedCertificate[];
    removedActive: boolean;
  }> {
    const config = await this.deps.getConfig();
    const normalizedPrimaryDomain = primaryDomain?.trim().toLowerCase() || "";
    const removed = (config.ssl.certificates || []).filter((certificate) => {
      if (certificate.source !== source) return false;
      if (!normalizedPrimaryDomain) return true;
      return certificate.primary_domain === normalizedPrimaryDomain;
    });

    if (removed.length === 0) {
      return { removed: [], removedActive: false };
    }

    const removedIds = new Set(removed.map((certificate) => certificate.id));
    const removedActive = removedIds.has(config.ssl.active_cert_id || "");
    config.ssl = {
      ...config.ssl,
      certificates: (config.ssl.certificates || []).filter(
        (certificate) => !removedIds.has(certificate.id),
      ),
    };
    config.ssl = mirrorActiveSSLCertificate(
      config.ssl,
      removedActive ? null : config.ssl.active_cert_id,
    );
    await this.deps.saveConfig(config);
    return { removed, removedActive };
  }

  async deleteSSLCertificatesBySourceRef(
    source: SSLCertificateSource,
    sourceRefId: string,
  ): Promise<{
    removed: SSLManagedCertificate[];
    removedActive: boolean;
  }> {
    const normalizedSourceRefId = sourceRefId.trim();
    if (!normalizedSourceRefId) {
      return { removed: [], removedActive: false };
    }

    const config = await this.deps.getConfig();
    const removed = (config.ssl.certificates || []).filter(
      (certificate) =>
        certificate.source === source &&
        certificate.source_ref_id === normalizedSourceRefId,
    );

    if (removed.length === 0) {
      return { removed: [], removedActive: false };
    }

    const removedIds = new Set(removed.map((certificate) => certificate.id));
    const removedActive = removedIds.has(config.ssl.active_cert_id || "");
    config.ssl = {
      ...config.ssl,
      certificates: (config.ssl.certificates || []).filter(
        (certificate) => !removedIds.has(certificate.id),
      ),
    };
    config.ssl = mirrorActiveSSLCertificate(
      config.ssl,
      removedActive ? null : config.ssl.active_cert_id,
    );
    await this.deps.saveConfig(config);
    return { removed, removedActive };
  }
}
