import {
  DEFAULT_GATEWAY_PORTAL_CONFIG,
  redis,
  type AppConfig,
  type GatewayPortalConfig,
} from "./redis";
import { goBackend } from "./go-backend";
import { isAnySubdomainRoutingMode } from "./reverse-proxy-submode";

export const GATEWAY_PORTAL_TITLE_HOST_RULES_PATCH_FLAG_KEY =
  "fn_knock:patch:gateway-portal-title-host-rules:v1";

export const normalizeGatewayPortalConfigForSync = (
  config?: Partial<GatewayPortalConfig> | null,
): GatewayPortalConfig => ({
  display_style: config?.display_style === "title" ? "title" : "domain",
});

export const isGatewayPortalTitleMode = (
  config: Pick<AppConfig, "gateway_portal">,
): boolean =>
  normalizeGatewayPortalConfigForSync(
    config.gateway_portal ?? DEFAULT_GATEWAY_PORTAL_CONFIG,
  ).display_style === "title";

export const syncGatewayPortalToGateway = async (
  config?: Partial<GatewayPortalConfig> | null,
): Promise<GatewayPortalConfig> => {
  const next = normalizeGatewayPortalConfigForSync(
    config ?? DEFAULT_GATEWAY_PORTAL_CONFIG,
  );
  const response = await goBackend.setGatewayPortalConfig(next);
  if (!response.success) {
    throw new Error(response.message || "同步传送门显示配置到网关失败");
  }
  return next;
};

export const syncGatewayPortalHostRulesIfTitleMode = async (
  config: Pick<
    AppConfig,
    "gateway_portal" | "host_mappings" | "run_type" | "reverse_proxy_submode"
  >,
): Promise<boolean> => {
  if (!isGatewayPortalTitleMode(config) || !isAnySubdomainRoutingMode(config)) {
    return false;
  }

  const response = await goBackend.setHostRules(config.host_mappings);
  if (!response.success) {
    throw new Error(response.message || "同步 Host 路由失败");
  }
  return true;
};

export const applyGatewayPortalTitleHostRulesPatchIfNeeded = async (
  config: Pick<
    AppConfig,
    "gateway_portal" | "host_mappings" | "run_type" | "reverse_proxy_submode"
  >,
): Promise<boolean> => {
  if (!isGatewayPortalTitleMode(config) || !isAnySubdomainRoutingMode(config)) {
    return false;
  }

  const patchFlag = await redis.get(
    GATEWAY_PORTAL_TITLE_HOST_RULES_PATCH_FLAG_KEY,
  );
  if (patchFlag === "1") {
    return false;
  }

  await syncGatewayPortalHostRulesIfTitleMode(config);

  try {
    await redis.set(GATEWAY_PORTAL_TITLE_HOST_RULES_PATCH_FLAG_KEY, "1");
  } catch (error) {
    console.error(
      "[gateway-portal] failed to mark title host-rules patch applied:",
      error,
    );
  }

  return true;
};
