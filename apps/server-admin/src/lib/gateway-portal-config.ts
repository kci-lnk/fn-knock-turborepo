import type { GatewayPortalConfig, GatewayPortalDisplayStyle } from "./redis";

export const normalizeGatewayPortalDisplayStyle = (
  value: unknown,
): GatewayPortalDisplayStyle => (value === "domain" ? "domain" : "title");

export const normalizeGatewayPortalConfigValue = (
  value?: Partial<GatewayPortalConfig> | null,
): GatewayPortalConfig => {
  const raw = value ?? {};

  return {
    enabled: raw.enabled !== false,
    display_style: normalizeGatewayPortalDisplayStyle(raw.display_style),
    show_app_icon: raw.show_app_icon !== false,
  };
};
