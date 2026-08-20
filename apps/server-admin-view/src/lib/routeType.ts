export const ROUTE_TYPE_TRANSLATION_KEYS = {
  path_rule: "admin.wafLogs.routeTypes.pathRule",
  host_rule: "admin.wafLogs.routeTypes.hostRule",
  host_location: "admin.wafLogs.routeTypes.hostLocation",
  stream_rule: "admin.wafLogs.routeTypes.streamRule",
  fn_connect: "admin.wafLogs.routeTypes.fnConnect",
  auth_proxy: "admin.wafLogs.routeTypes.authProxy",
  certificate_deploy: "admin.wafLogs.routeTypes.certificateDeploy",
  select: "admin.wafLogs.routeTypes.select",
  wol: "admin.wafLogs.routeTypes.wol",
  preflight: "admin.wafLogs.routeTypes.preflight",
  slash_redirect: "admin.wafLogs.routeTypes.slashRedirect",
  favicon: "admin.wafLogs.routeTypes.favicon",
  toolbar_asset: "admin.wafLogs.routeTypes.toolbarAsset",
  crawler_blocker: "admin.wafLogs.routeTypes.crawlerBlocker",
  general_blacklist: "admin.wafLogs.routeTypes.generalBlacklist",
  visibility: "admin.wafLogs.routeTypes.visibility",
  protocol_misdirected: "admin.wafLogs.routeTypes.protocolMisdirected",
  host_unavailable: "admin.wafLogs.routeTypes.hostUnavailable",
  default_host_redirect: "admin.wafLogs.routeTypes.defaultHostRedirect",
  unmatched_route_blocked: "admin.wafLogs.routeTypes.unmatchedRouteBlocked",
  not_found: "admin.wafLogs.routeTypes.notFound",
} as const;

export type RouteTypeTranslator = (key: string) => string;

export const routeTypeLabel = (
  value: string | undefined,
  translate: RouteTypeTranslator,
) => {
  const normalized = value?.trim().toLowerCase() || "";
  if (!normalized) return "-";
  const key =
    ROUTE_TYPE_TRANSLATION_KEYS[
      normalized as keyof typeof ROUTE_TYPE_TRANSLATION_KEYS
    ];
  return key ? translate(key) : value || "-";
};
