import type { GatewayPortalConfig, GatewayPortalVersion } from "../types";

export const normalizeGatewayPortalVersion = (
  version: unknown,
): GatewayPortalVersion => (version === "v2" ? "v2" : "v1");

export const normalizeGatewayPortalConfig = (
  portal?: Partial<GatewayPortalConfig> | null,
): GatewayPortalConfig => ({
  enabled: portal?.enabled !== false,
  display_style: portal?.display_style === "domain" ? "domain" : "title",
  show_app_icon: portal?.show_app_icon !== false,
  show_wol: portal?.show_wol !== false,
  icon_drag_mode: portal?.icon_drag_mode === "free" ? "free" : "corners",
  version: normalizeGatewayPortalVersion(portal?.version),
});

export const buildGatewayPortalVersionPatch = (
  version: GatewayPortalVersion,
) => ({
  portal: {
    version: normalizeGatewayPortalVersion(version),
  },
});
