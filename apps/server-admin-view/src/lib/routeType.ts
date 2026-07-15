export const ROUTE_TYPE_TRANSLATION_KEYS = {
  path_rule: "admin.wafLogs.routeTypes.pathRule",
  host_rule: "admin.wafLogs.routeTypes.hostRule",
  host_location: "admin.wafLogs.routeTypes.hostLocation",
  stream_rule: "admin.wafLogs.routeTypes.streamRule",
  auth_proxy: "admin.wafLogs.routeTypes.authProxy",
  select: "admin.wafLogs.routeTypes.select",
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
  not_found: "admin.wafLogs.routeTypes.notFound",
} as const;

export type RouteTypeTranslator = (key: string) => string;

export const routeTypeLabel = (
  value: string | undefined,
  translate: RouteTypeTranslator,
) => {
  if (!value) return "-";
  const key =
    ROUTE_TYPE_TRANSLATION_KEYS[
      value as keyof typeof ROUTE_TYPE_TRANSLATION_KEYS
    ];
  return key ? translate(key) : value;
};
