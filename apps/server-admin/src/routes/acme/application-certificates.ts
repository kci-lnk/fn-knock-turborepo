import { rm } from "node:fs/promises";
import { join } from "node:path";
import {
  configManager,
  type AcmeApplication,
  type AcmeApplicationSaveResult,
} from "../../lib/redis";
import { syncSSLDeploymentToGateway } from "../../lib/ssl-gateway";

export const getUsableIssuedCertificateForApplication = async (
  application: AcmeApplication,
) => {
  const issuedCertificate = await configManager.getAcmeIssuedCertificate(
    application.id,
  );
  if (
    !configManager.isAcmeIssuedCertificateCompatible(
      application,
      issuedCertificate,
    )
  ) {
    return null;
  }
  return issuedCertificate;
};

export const getStatusCertificate = async () => {
  const applications = await configManager.listAcmeApplications();
  for (const application of applications) {
    const issuedCertificate =
      await getUsableIssuedCertificateForApplication(application);
    if (!issuedCertificate) continue;
    return {
      primaryDomain: issuedCertificate.primaryDomain,
      info: issuedCertificate.certInfo,
    };
  }
  return null;
};

export const syncGatewayIfAcmeLibraryRemoved = async (input: {
  removedActive: boolean;
  removedCount: number;
}) => {
  if (!input.removedActive && input.removedCount <= 0) return;
  const currentConfig = await configManager.getConfig();
  if (
    input.removedActive ||
    (input.removedCount > 0 &&
      currentConfig.ssl.deployment_mode === "multi_sni")
  ) {
    await syncSSLDeploymentToGateway(currentConfig);
  }
};

export const syncGatewayIfAcmeApplicationSaveRemovedLibrary = async (
  saved: Pick<
    AcmeApplicationSaveResult,
    "removedActiveLibraryCertificate" | "removedLibraryCertificates"
  >,
) => {
  await syncGatewayIfAcmeLibraryRemoved({
    removedActive: saved.removedActiveLibraryCertificate,
    removedCount: saved.removedLibraryCertificates.length,
  });
};

export const deleteAcmeApplicationCertificate = async (
  applicationId: string,
  notFoundMessage: string,
) => {
  const application = await configManager.getAcmeApplication(applicationId);
  if (!application) {
    throw new Error(notFoundMessage);
  }

  const issuedCertificate =
    await configManager.getAcmeIssuedCertificate(applicationId);
  const deletedFromLibrary =
    await configManager.deleteSSLCertificatesBySourceRef("acme", applicationId);
  await configManager.deleteAcmeIssuedCertificate(applicationId);

  const domainsToRemove = new Set(
    [application.primaryDomain, issuedCertificate?.primaryDomain].filter(
      (value): value is string => Boolean(value),
    ),
  );

  for (const domain of domainsToRemove) {
    await rm(join(process.cwd(), "data", "ssl", domain), {
      recursive: true,
      force: true,
    });
  }

  await syncGatewayIfAcmeLibraryRemoved({
    removedActive: deletedFromLibrary.removedActive,
    removedCount: deletedFromLibrary.removed.length,
  });

  return {
    application,
    issuedCertificate,
    deletedFromLibrary,
  };
};

export const deleteAcmeApplication = async (
  applicationId: string,
  notFoundMessage: string,
) => {
  const deleted = await configManager.deleteAcmeApplication(applicationId);
  if (!deleted) {
    throw new Error(notFoundMessage);
  }

  await syncGatewayIfAcmeLibraryRemoved({
    removedActive: deleted.removedActiveLibraryCertificate,
    removedCount: deleted.removedLibraryCertificates.length,
  });

  return deleted;
};
