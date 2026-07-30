import type {
  GatewayUnmatchedRouteBehavior,
  GatewayUnmatchedRouteConfig,
  GatewayUpstreamErrorDetail,
} from "@/types";

export const normalizeGatewayUnmatchedRouteBehavior = (
  value?: string | null,
): GatewayUnmatchedRouteBehavior =>
  value === "reset_connection" ? "reset_connection" : "error_page";

export const isDefaultDomainAvailableForBehavior = (
  value?: string | null,
): boolean => normalizeGatewayUnmatchedRouteBehavior(value) === "error_page";

export const normalizeGatewayUpstreamErrorDetail = (
  value?: string | null,
): GatewayUpstreamErrorDetail =>
  value === "more" || value === "reset_connection" ? value : "less";

export const buildGatewayUnmatchedRoutePatch = (
  behavior: GatewayUnmatchedRouteBehavior,
  upstreamErrorDetail: GatewayUpstreamErrorDetail = "less",
): { unmatched_route: GatewayUnmatchedRouteConfig } => ({
  unmatched_route: {
    behavior: normalizeGatewayUnmatchedRouteBehavior(behavior),
    upstream_error_detail:
      normalizeGatewayUpstreamErrorDetail(upstreamErrorDetail),
  },
});
