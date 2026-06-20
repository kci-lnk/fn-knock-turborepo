import { Elysia } from "elysia";
import { adminAuthSettingsRoutes } from "./auth-settings";
import { adminGatewaySettingsRoutes } from "./gateway-settings";
import { adminMaintenanceRoutes } from "./maintenance";
import { adminPanelRoutes } from "./panel";
import { adminProxyMappingsRoutes } from "./proxy-mappings";
import { adminRuntimeConfigRoutes } from "./runtime-config";
import { adminSecurityRoutes } from "./security";
import { adminSessionRoutes } from "./sessions";

export const adminRoutes = new Elysia({
  prefix: "/api/admin",
  tags: ["Admin"],
})
  .use(adminPanelRoutes)
  .use(adminRuntimeConfigRoutes)
  .use(adminGatewaySettingsRoutes)
  .use(adminProxyMappingsRoutes)
  .use(adminAuthSettingsRoutes)
  .use(adminMaintenanceRoutes)
  .use(adminSecurityRoutes)
  .use(adminSessionRoutes);
