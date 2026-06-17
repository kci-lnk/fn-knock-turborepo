import {
  DEFAULT_GATEWAY_PORTAL_CONFIG,
  redis,
  type AppConfig,
  type GatewayPortalConfig,
} from "./redis";
import { goBackend } from "./go-backend";
import { isAnySubdomainRoutingMode } from "./reverse-proxy-submode";
import { tDefault } from "./i18n";

export const GATEWAY_PORTAL_TITLE_HOST_RULES_PATCH_FLAG_KEY =
  "fn_knock:patch:gateway-portal-title-host-rules:v1";
export const GATEWAY_PORTAL_ICON_HOST_RULES_PATCH_FLAG_KEY =
  "fn_knock:patch:gateway-portal-icon-host-rules:v1";
const gatewayPortalT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.gatewayPortal.${key}`, params);

export const normalizeGatewayPortalConfigForSync = (
  config?: Partial<GatewayPortalConfig> | null,
): GatewayPortalConfig => ({
  display_style: config?.display_style === "title" ? "title" : "domain",
  show_app_icon: config?.show_app_icon === true,
});

export const isGatewayPortalTitleMode = (
  config: Pick<AppConfig, "gateway_portal">,
): boolean =>
  normalizeGatewayPortalConfigForSync(
    config.gateway_portal ?? DEFAULT_GATEWAY_PORTAL_CONFIG,
  ).display_style === "title";

export const isGatewayPortalAppIconMode = (
  config: Pick<AppConfig, "gateway_portal">,
): boolean =>
  normalizeGatewayPortalConfigForSync(
    config.gateway_portal ?? DEFAULT_GATEWAY_PORTAL_CONFIG,
  ).show_app_icon === true;

export const syncGatewayPortalToGateway = async (
  config?: Partial<GatewayPortalConfig> | null,
): Promise<GatewayPortalConfig> => {
  const next = normalizeGatewayPortalConfigForSync(
    config ?? DEFAULT_GATEWAY_PORTAL_CONFIG,
  );
  const response = await goBackend.setGatewayPortalConfig(next);
  if (!response.success) {
    throw new Error(response.message || gatewayPortalT("syncConfigFailed"));
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
    throw new Error(response.message || gatewayPortalT("syncHostRulesFailed"));
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

export const applyGatewayPortalIconHostRulesPatchIfNeeded = async (
  config: Pick<
    AppConfig,
    "gateway_portal" | "host_mappings" | "run_type" | "reverse_proxy_submode"
  >,
): Promise<boolean> => {
  if (
    !isGatewayPortalAppIconMode(config) ||
    !isAnySubdomainRoutingMode(config)
  ) {
    return false;
  }

  const patchFlag = await redis.get(
    GATEWAY_PORTAL_ICON_HOST_RULES_PATCH_FLAG_KEY,
  );
  if (patchFlag === "1") {
    return false;
  }

  const response = await goBackend.setHostRules(config.host_mappings);
  if (!response.success) {
    throw new Error(response.message || gatewayPortalT("syncHostRulesFailed"));
  }

  try {
    await redis.set(GATEWAY_PORTAL_ICON_HOST_RULES_PATCH_FLAG_KEY, "1");
  } catch (error) {
    console.error(
      "[gateway-portal] failed to mark icon host-rules patch applied:",
      error,
    );
  }

  return true;
};
