import type {
  GatewayPortalConfig,
  GatewayPortalDisplayStyle,
  GatewayPortalIconDragMode,
} from "./redis";

export const normalizeGatewayPortalDisplayStyle = (
  value: unknown,
): GatewayPortalDisplayStyle => (value === "domain" ? "domain" : "title");

export const normalizeGatewayPortalIconDragMode = (
  value: unknown,
): GatewayPortalIconDragMode => (value === "free" ? "free" : "corners");

export const normalizeGatewayPortalConfigValue = (
  value?: Partial<GatewayPortalConfig> | null,
): GatewayPortalConfig => {
  const raw = value ?? {};

  return {
    enabled: raw.enabled !== false,
    display_style: normalizeGatewayPortalDisplayStyle(raw.display_style),
    show_app_icon: raw.show_app_icon !== false,
    icon_drag_mode: normalizeGatewayPortalIconDragMode(raw.icon_drag_mode),
  };
};
