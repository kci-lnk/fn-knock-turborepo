import type { GatewayLogTranslator } from "./gateway-request-log-types";

export const getTodayString = () => {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
};

export const LIMIT_OPTIONS = ["10", "20", "50", "100"] as const;

export const STATUS_FILTER_OPTIONS = [
  { value: "all", labelKey: "admin.gatewayRequestLogs.statusFilters.all" },
  {
    value: "2xx",
    labelKey: "admin.gatewayRequestLogs.statusFilters.success2xx",
  },
  {
    value: "3xx",
    labelKey: "admin.gatewayRequestLogs.statusFilters.redirect3xx",
  },
  {
    value: "4xx",
    labelKey: "admin.gatewayRequestLogs.statusFilters.client4xx",
  },
  {
    value: "5xx",
    labelKey: "admin.gatewayRequestLogs.statusFilters.server5xx",
  },
  {
    value: "401",
    labelKey: "admin.gatewayRequestLogs.statusFilters.unauthorized401",
  },
  {
    value: "403",
    labelKey: "admin.gatewayRequestLogs.statusFilters.forbidden403",
  },
  {
    value: "404",
    labelKey: "admin.gatewayRequestLogs.statusFilters.notFound404",
  },
  {
    value: "500",
    labelKey: "admin.gatewayRequestLogs.statusFilters.serverError500",
  },
  {
    value: "502",
    labelKey: "admin.gatewayRequestLogs.statusFilters.badGateway502",
  },
  {
    value: "503",
    labelKey: "admin.gatewayRequestLogs.statusFilters.unavailable503",
  },
] as const;

export const LOGIN_FILTER_OPTIONS = [
  { value: "all", labelKey: "admin.gatewayRequestLogs.loginFilters.all" },
  { value: "true", labelKey: "admin.gatewayRequestLogs.loginFilters.loggedIn" },
  {
    value: "false",
    labelKey: "admin.gatewayRequestLogs.loginFilters.notLoggedIn",
  },
] as const;

export const WAF_FILTER_OPTIONS = [
  { value: "all", labelKey: "admin.gatewayRequestLogs.wafFilters.all" },
  { value: "has_waf", labelKey: "admin.gatewayRequestLogs.wafFilters.hasWaf" },
  { value: "none", labelKey: "admin.gatewayRequestLogs.wafFilters.none" },
] as const;

export const UNRECORDED_CREDENTIAL_FILTER = "__unrecorded__";

export type GatewayStatusFilterValue =
  (typeof STATUS_FILTER_OPTIONS)[number]["value"];
export type GatewayLoginFilterValue =
  (typeof LOGIN_FILTER_OPTIONS)[number]["value"];
export type GatewayWAFFilterValue =
  (typeof WAF_FILTER_OPTIONS)[number]["value"];

export const getGatewayLogOptionLabel = <
  TOption extends { value: string; labelKey: string },
>(
  options: readonly TOption[],
  value: string,
  fallbackLabelKey: string,
  t: GatewayLogTranslator,
) =>
  t(options.find((item) => item.value === value)?.labelKey || fallbackLabelKey);
