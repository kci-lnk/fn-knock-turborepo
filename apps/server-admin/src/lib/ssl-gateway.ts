import { goBackend, type SSLDeployedCertificate, type SSLDeploymentMode } from "./go-backend";
import { configManager, type AppConfig, type SSLManagedCertificate } from "./redis";

const normalizeDeploymentMode = (
  value: SSLDeploymentMode | undefined | null,
): SSLDeploymentMode => (value === "multi_sni" ? "multi_sni" : "single_active");

const findActiveCertificate = (
  config: Pick<AppConfig, "ssl">,
): SSLManagedCertificate | null => {
  const activeId = config.ssl.active_cert_id?.trim() || "";
  if (!activeId) return null;
  return (
    (config.ssl.certificates || []).find((certificate) => certificate.id === activeId) ||
    null
  );
};

const buildGatewayCertificatePayload = (
  certificate: SSLManagedCertificate,
  isDefault: boolean,
): SSLDeployedCertificate => ({
  id: certificate.id,
  label: certificate.label,
  cert: certificate.cert,
  key: certificate.key,
  is_default: isDefault,
});

export const buildGatewaySSLDeployment = (
  config: Pick<AppConfig, "ssl">,
): {
  deployment_mode: SSLDeploymentMode;
  certificates: SSLDeployedCertificate[];
} => {
  const deploymentMode = normalizeDeploymentMode(config.ssl.deployment_mode);
  const certificates = config.ssl.certificates || [];
  const activeCertificate = findActiveCertificate(config);

  if (deploymentMode !== "multi_sni") {
    return {
      deployment_mode: "single_active",
      certificates: activeCertificate
        ? [buildGatewayCertificatePayload(activeCertificate, true)]
        : [],
    };
  }

  const ordered: SSLManagedCertificate[] = [];
  const seen = new Set<string>();
  if (activeCertificate) {
    ordered.push(activeCertificate);
    seen.add(activeCertificate.id);
  }
  for (const certificate of certificates) {
    if (seen.has(certificate.id)) continue;
    seen.add(certificate.id);
    ordered.push(certificate);
  }

  return {
    deployment_mode: "multi_sni",
    certificates: ordered.map((certificate, index) =>
      buildGatewayCertificatePayload(
        certificate,
        activeCertificate
          ? certificate.id === activeCertificate.id
          : index === 0,
      ),
    ),
  };
};

export const syncSSLDeploymentToGateway = async (
  config?: AppConfig,
): Promise<void> => {
  const nextConfig = config || (await configManager.getConfig());
  const deployment = buildGatewaySSLDeployment(nextConfig);

  if (deployment.certificates.length === 0) {
    const resp = await goBackend.clearSSL();
    if (!resp.success) {
      throw new Error(resp.message || "清除网关证书失败");
    }
    return;
  }

  const resp = await goBackend.setSSLDeployment(deployment);
  if (!resp.success) {
    throw new Error(resp.message || "同步网关证书失败");
  }
};

