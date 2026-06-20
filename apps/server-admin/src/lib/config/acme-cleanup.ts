import { rm } from "node:fs/promises";
import { join } from "node:path";
import type {
  AcmeIssuedCertificate,
  SSLManagedCertificate,
} from "./types";

type DeletedSSLCertificates = {
  removed: SSLManagedCertificate[];
  removedActive: boolean;
};

type DeleteSSLCertificatesBySourceRef = (
  source: "acme",
  sourceRefId: string,
) => Promise<DeletedSSLCertificates>;

type DeleteSSLCertificatesBySource = (
  source: "acme",
  primaryDomain: string,
) => Promise<DeletedSSLCertificates>;

export interface AcmeArtifactCleanupResult {
  removedLibraryCertificates: SSLManagedCertificate[];
  removedActiveLibraryCertificate: boolean;
  removedDomains: string[];
}

const mergeRemovedLibraryCertificates = (
  primary: SSLManagedCertificate[],
  secondary: SSLManagedCertificate[],
): SSLManagedCertificate[] => [
  ...primary,
  ...secondary.filter(
    (certificate) => !primary.some((item) => item.id === certificate.id),
  ),
];

const collectRemovedDomains = (
  primaryDomain: string,
  deletedIssuedCertificate: AcmeIssuedCertificate | null,
): string[] =>
  Array.from(
    new Set(
      [
        primaryDomain,
        deletedIssuedCertificate?.primaryDomain,
      ].filter((value): value is string => Boolean(value)),
    ),
  );

const removeAcmeDomainArtifacts = async (
  domains: string[],
  deleteAcmeCert: (domain: string) => Promise<void>,
): Promise<void> => {
  for (const domain of domains) {
    await deleteAcmeCert(domain);
    await rm(join(process.cwd(), "data", "ssl", domain), {
      recursive: true,
      force: true,
    });
  }
};

export const cleanupAcmeApplicationArtifacts = async (input: {
  applicationId: string;
  deleteAcmeCert: (domain: string) => Promise<void>;
  deletedIssuedCertificate: AcmeIssuedCertificate | null;
  deleteSSLCertificatesBySource: DeleteSSLCertificatesBySource;
  deleteSSLCertificatesBySourceRef: DeleteSSLCertificatesBySourceRef;
  primaryDomain: string;
}): Promise<AcmeArtifactCleanupResult> => {
  const deletedBySourceRef = await input.deleteSSLCertificatesBySourceRef(
    "acme",
    input.applicationId,
  );
  const deletedByPrimaryDomain = await input.deleteSSLCertificatesBySource(
    "acme",
    input.primaryDomain,
  );
  const removedLibraryCertificates = mergeRemovedLibraryCertificates(
    deletedBySourceRef.removed,
    deletedByPrimaryDomain.removed,
  );
  const removedDomains = collectRemovedDomains(
    input.primaryDomain,
    input.deletedIssuedCertificate,
  );

  await removeAcmeDomainArtifacts(removedDomains, input.deleteAcmeCert);

  return {
    removedLibraryCertificates,
    removedActiveLibraryCertificate:
      deletedBySourceRef.removedActive || deletedByPrimaryDomain.removedActive,
    removedDomains,
  };
};
