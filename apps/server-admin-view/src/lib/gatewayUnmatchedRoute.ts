import type {
  GatewayUnmatchedRouteBehavior,
  GatewayUnmatchedRouteConfig,
} from "@/types";

export const normalizeGatewayUnmatchedRouteBehavior = (
  value?: string | null,
): GatewayUnmatchedRouteBehavior =>
  value === "reset_connection" ? "reset_connection" : "error_page";

export const isDefaultDomainAvailableForBehavior = (
  value?: string | null,
): boolean => normalizeGatewayUnmatchedRouteBehavior(value) === "error_page";

export const buildGatewayUnmatchedRoutePatch = (
  behavior: GatewayUnmatchedRouteBehavior,
): { unmatched_route: GatewayUnmatchedRouteConfig } => ({
  unmatched_route: {
    behavior: normalizeGatewayUnmatchedRouteBehavior(behavior),
  },
});
