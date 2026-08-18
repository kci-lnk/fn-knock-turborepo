import type {
  NotificationDeliveryStatus,
  NotificationGroupBy,
  SystemEventLevel,
  SystemEventSource,
  SystemEventType,
} from "../../types";

export const SYSTEM_EVENT_TYPE_OPTIONS: Array<{
  value: SystemEventType;
  labelKey: string;
}> = [
  {
    value: "FN_EVENT_AUTH_LOGIN_SUCCESS",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_AUTH_LOGIN_SUCCESS",
  },
  {
    value: "FN_EVENT_AUTH_LOGOUT",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_AUTH_LOGOUT",
  },
  {
    value: "FN_EVENT_AUTH_LOGIN_FAILURE",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_AUTH_LOGIN_FAILURE",
  },
  {
    value: "FN_EVENT_AUTH_SESSION_IP_DRIFT",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_AUTH_SESSION_IP_DRIFT",
  },
  {
    value: "FN_EVENT_SECURITY_SCANNER_BLOCKED",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_SECURITY_SCANNER_BLOCKED",
  },
  {
    value: "FN_EVENT_DDNS_UPDATE_COMPLETED",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_DDNS_UPDATE_COMPLETED",
  },
  {
    value: "FN_EVENT_WOL_WAKE_COMPLETED",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_WOL_WAKE_COMPLETED",
  },
  {
    value: "FN_EVENT_PANEL_SYNC_FAILED" as SystemEventType,
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_PANEL_SYNC_FAILED",
  },
  {
    value: "FN_EVENT_PANEL_SYNC_RECOVERED" as SystemEventType,
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_PANEL_SYNC_RECOVERED",
  },
  {
    value: "FN_EVENT_GATEWAY_THROTTLE_BLOCKED",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_GATEWAY_THROTTLE_BLOCKED",
  },
  {
    value: "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED",
    labelKey:
      "admin.eventCenter.eventTypes.FN_EVENT_GATEWAY_VISIBILITY_BLOCKED",
  },
  {
    value: "FN_EVENT_WAF_BLOCKED",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_WAF_BLOCKED",
  },
  {
    value: "FN_EVENT_SSH_LOGIN_SUCCESS",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_SSH_LOGIN_SUCCESS",
  },
  {
    value: "FN_EVENT_SSH_LOGIN_FAILURE",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_SSH_LOGIN_FAILURE",
  },
  {
    value: "FN_EVENT_SSH_IP_BLOCKED",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_SSH_IP_BLOCKED",
  },
  {
    value: "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE",
    labelKey:
      "admin.eventCenter.eventTypes.FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE",
  },
  {
    value: "FN_EVENT_SYSTEM_CPU_ALERT",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_SYSTEM_CPU_ALERT",
  },
  {
    value: "FN_EVENT_SYSTEM_CPU_RECOVERED",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_SYSTEM_CPU_RECOVERED",
  },
  {
    value: "FN_EVENT_SYSTEM_MEMORY_ALERT",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_SYSTEM_MEMORY_ALERT",
  },
  {
    value: "FN_EVENT_SYSTEM_MEMORY_RECOVERED",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_SYSTEM_MEMORY_RECOVERED",
  },
  {
    value: "FN_EVENT_TUNNEL_FRP_CONNECTED",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_TUNNEL_FRP_CONNECTED",
  },
  {
    value: "FN_EVENT_TUNNEL_FRP_DISCONNECTED",
    labelKey: "admin.eventCenter.eventTypes.FN_EVENT_TUNNEL_FRP_DISCONNECTED",
  },
  {
    value: "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED",
    labelKey:
      "admin.eventCenter.eventTypes.FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED",
  },
  {
    value: "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED",
    labelKey:
      "admin.eventCenter.eventTypes.FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED",
  },
  ...[
    "FN_EVENT_RUNTIME_STARTED",
    "FN_EVENT_RUNTIME_STOPPED",
    "FN_EVENT_RUNTIME_RESTARTED",
    "FN_EVENT_RUNTIME_HEALTH_FAILED",
    "FN_EVENT_RUNTIME_RECOVERED",
    "FN_EVENT_RUNTIME_ABNORMAL_EXIT",
  ].map((value) => ({
    value: value as SystemEventType,
    labelKey: `admin.eventCenter.eventTypes.${value}`,
  })),
];

export const SYSTEM_EVENT_TYPE_FILTER_OPTIONS: Array<{
  value: SystemEventType | "all";
  labelKey: string;
}> = [
  { value: "all", labelKey: "admin.eventCenter.filters.allEvents" },
  ...SYSTEM_EVENT_TYPE_OPTIONS,
];

export const SYSTEM_EVENT_LEVEL_OPTIONS: Array<{
  value: SystemEventLevel;
  labelKey: string;
}> = [
  { value: "INFO", labelKey: "admin.eventCenter.levels.INFO" },
  { value: "WARN", labelKey: "admin.eventCenter.levels.WARN" },
  { value: "ERROR", labelKey: "admin.eventCenter.levels.ERROR" },
  { value: "CRITICAL", labelKey: "admin.eventCenter.levels.CRITICAL" },
];

export const SYSTEM_EVENT_LEVEL_FILTER_OPTIONS: Array<{
  value: SystemEventLevel | "all";
  labelKey: string;
}> = [
  { value: "all", labelKey: "admin.eventCenter.filters.allLevels" },
  ...SYSTEM_EVENT_LEVEL_OPTIONS,
];

export const SYSTEM_EVENT_SOURCE_OPTIONS: Array<{
  value: SystemEventSource;
  labelKey: string;
}> = [
  {
    value: "SERVER_ADMIN",
    labelKey: "admin.eventCenter.sources.SERVER_ADMIN",
  },
  {
    value: "GO_REAUTH_PROXY",
    labelKey: "admin.eventCenter.sources.GO_REAUTH_PROXY",
  },
  {
    value: "SYSTEM_MONITOR",
    labelKey: "admin.eventCenter.sources.SYSTEM_MONITOR",
  },
  {
    value: "RUNTIME_MONITOR",
    labelKey: "admin.eventCenter.sources.RUNTIME_MONITOR",
  },
];

export const SYSTEM_EVENT_SOURCE_FILTER_OPTIONS: Array<{
  value: SystemEventSource | "all";
  labelKey: string;
}> = [
  { value: "all", labelKey: "admin.eventCenter.filters.allSystems" },
  ...SYSTEM_EVENT_SOURCE_OPTIONS,
];

export const NOTIFICATION_GROUP_BY_OPTIONS: Array<{
  value: NotificationGroupBy;
  labelKey: string;
}> = [
  { value: "GLOBAL", labelKey: "admin.eventCenter.groupBy.GLOBAL" },
  { value: "IP", labelKey: "admin.eventCenter.groupBy.IP" },
  { value: "SESSION", labelKey: "admin.eventCenter.groupBy.SESSION" },
  { value: "SUBJECT", labelKey: "admin.eventCenter.groupBy.SUBJECT" },
  { value: "HOSTNAME", labelKey: "admin.eventCenter.groupBy.HOSTNAME" },
  { value: "PROVIDER", labelKey: "admin.eventCenter.groupBy.PROVIDER" },
];

export const DEFAULT_GROUP_BY_BY_EVENT_TYPE: Record<
  SystemEventType,
  NotificationGroupBy
> = {
  FN_EVENT_AUTH_LOGIN_SUCCESS: "GLOBAL",
  FN_EVENT_AUTH_LOGOUT: "GLOBAL",
  FN_EVENT_AUTH_LOGIN_FAILURE: "IP",
  FN_EVENT_AUTH_SESSION_IP_DRIFT: "SESSION",
  FN_EVENT_SECURITY_SCANNER_BLOCKED: "IP",
  FN_EVENT_DDNS_UPDATE_COMPLETED: "PROVIDER",
  FN_EVENT_WOL_WAKE_COMPLETED: "SUBJECT",
  FN_EVENT_WOL_SHUTDOWN_COMPLETED: "SUBJECT",
  FN_EVENT_PANEL_SYNC_FAILED: "SUBJECT",
  FN_EVENT_PANEL_SYNC_RECOVERED: "SUBJECT",
  FN_EVENT_GATEWAY_THROTTLE_BLOCKED: "IP",
  FN_EVENT_GATEWAY_VISIBILITY_BLOCKED: "GLOBAL",
  FN_EVENT_WAF_BLOCKED: "IP",
  FN_EVENT_SSH_LOGIN_SUCCESS: "IP",
  FN_EVENT_SSH_LOGIN_FAILURE: "IP",
  FN_EVENT_SSH_IP_BLOCKED: "IP",
  FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE: "SUBJECT",
  FN_EVENT_SYSTEM_CPU_ALERT: "HOSTNAME",
  FN_EVENT_SYSTEM_CPU_RECOVERED: "HOSTNAME",
  FN_EVENT_SYSTEM_MEMORY_ALERT: "HOSTNAME",
  FN_EVENT_SYSTEM_MEMORY_RECOVERED: "HOSTNAME",
  FN_EVENT_TUNNEL_FRP_CONNECTED: "SUBJECT",
  FN_EVENT_TUNNEL_FRP_DISCONNECTED: "SUBJECT",
  FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED: "SUBJECT",
  FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED: "SUBJECT",
  FN_EVENT_RUNTIME_STARTED: "SUBJECT",
  FN_EVENT_RUNTIME_STOPPED: "SUBJECT",
  FN_EVENT_RUNTIME_RESTARTED: "SUBJECT",
  FN_EVENT_RUNTIME_HEALTH_FAILED: "SUBJECT",
  FN_EVENT_RUNTIME_RECOVERED: "SUBJECT",
  FN_EVENT_RUNTIME_ABNORMAL_EXIT: "SUBJECT",
};

export const NOTIFICATION_DELIVERY_STATUS_OPTIONS: Array<{
  value: NotificationDeliveryStatus | "all";
  labelKey: string;
}> = [
  { value: "all", labelKey: "admin.eventCenter.deliveryStatus.all" },
  { value: "queued", labelKey: "admin.eventCenter.deliveryStatus.queued" },
  { value: "sending", labelKey: "admin.eventCenter.deliveryStatus.sending" },
  { value: "success", labelKey: "admin.eventCenter.deliveryStatus.success" },
  { value: "failed", labelKey: "admin.eventCenter.deliveryStatus.failed" },
  { value: "gave_up", labelKey: "admin.eventCenter.deliveryStatus.gave_up" },
  { value: "skipped", labelKey: "admin.eventCenter.deliveryStatus.skipped" },
];

export const formatSystemEventTypeLabel = (type: SystemEventType) =>
  SYSTEM_EVENT_TYPE_OPTIONS.find((item) => item.value === type)?.labelKey ||
  type;

export const formatSystemEventLevelLabel = (level: SystemEventLevel) =>
  SYSTEM_EVENT_LEVEL_OPTIONS.find((item) => item.value === level)?.labelKey ||
  level;

export const formatSystemEventSourceLabel = (source: SystemEventSource) =>
  SYSTEM_EVENT_SOURCE_OPTIONS.find((item) => item.value === source)?.labelKey ||
  source;

export const formatNotificationGroupByLabel = (value: NotificationGroupBy) =>
  NOTIFICATION_GROUP_BY_OPTIONS.find((item) => item.value === value)
    ?.labelKey || value;

export const formatDeliveryStatusLabel = (value: NotificationDeliveryStatus) =>
  NOTIFICATION_DELIVERY_STATUS_OPTIONS.find((item) => item.value === value)
    ?.labelKey || value;
