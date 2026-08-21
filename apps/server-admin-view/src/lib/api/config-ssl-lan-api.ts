import type { components as ApiContractComponents } from "@fn-knock/api-contract";
import { apiClient } from "./client";

type LanCertificateDeployment =
  ApiContractComponents["schemas"]["LanCertificateDeploymentData"];
type LanCertificateDeploymentUpdateBody =
  ApiContractComponents["schemas"]["LanCertificateDeploymentUpdateBodyData"];

export const configSslLanApi = {
  async getLanCertificateDeployment(): Promise<LanCertificateDeployment> {
    const res = await apiClient.get("/ssl/external-bindings/lan");
    return res.data.data;
  },
  async updateLanCertificateDeployment(
    update: LanCertificateDeploymentUpdateBody,
  ): Promise<LanCertificateDeployment> {
    const payload = update satisfies LanCertificateDeploymentUpdateBody;
    const res = await apiClient.put("/ssl/external-bindings/lan", payload);
    return res.data.data;
  },
};
