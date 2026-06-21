import type {
  SystemEventLevel,
  SystemEventSource,
  SystemEventSubjectKind,
  SystemEventType,
} from "./constants";
export type { SystemEventType } from "./constants";
import {
  FN_EVENT_AUTH_LOGIN_FAILURE,
  FN_EVENT_AUTH_LOGIN_SUCCESS,
  FN_EVENT_AUTH_LOGOUT,
  FN_EVENT_AUTH_SESSION_IP_DRIFT,
  FN_EVENT_DDNS_UPDATE_COMPLETED,
  FN_EVENT_GATEWAY_THROTTLE_BLOCKED,
  FN_EVENT_WAF_BLOCKED,
  FN_EVENT_SSH_IP_BLOCKED,
  FN_EVENT_SSH_LOGIN_FAILURE,
  FN_EVENT_SSH_LOGIN_SUCCESS,
  FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE,
  FN_EVENT_SECURITY_SCANNER_BLOCKED,
  FN_EVENT_SYSTEM_CPU_ALERT,
  FN_EVENT_SYSTEM_CPU_RECOVERED,
  FN_EVENT_SYSTEM_MEMORY_ALERT,
  FN_EVENT_SYSTEM_MEMORY_RECOVERED,
  FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED,
  FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED,
  FN_EVENT_TUNNEL_FRP_CONNECTED,
  FN_EVENT_TUNNEL_FRP_DISCONNECTED,
} from "./constants";

export type AuthMethod = "TOTP" | "PASSKEY" | "OIDC";

export type SystemEventSessionDriftSource =
  | "proxy-session"
  | "fnos-token"
  | "session-refresh"
  | "browser-session";

export type SystemEventSubject = {
  kind: SystemEventSubjectKind;
  id: string;
};

export type SystemEventAuthLoginSuccessPayload = {
  session_id: string;
  auth_method: AuthMethod;
  auth_provider_name?: string;
  credential_id: string;
  credential_name: string;
  linked_totp_name?: string;
  session_comment?: string;
  grant_type: "browser_session" | "login_ip_grant";
  post_login_ip_grant_mode?: "follow_session" | "disabled" | "custom" | null;
  whitelist_record_id?: string | null;
  ip: string;
  ip_location?: string;
  user_agent: string;
  remember_me: boolean;
  expires_at: string;
};

export type SystemEventAuthLogoutPayload = {
  session_id: string;
  auth_method: AuthMethod;
  credential_id: string;
  credential_name: string;
  linked_totp_name?: string;
  session_comment?: string;
  ip: string;
  ip_location?: string;
  user_agent: string;
  login_time?: string;
  logout_source: "user_logout" | "admin_session_delete";
};

export type SystemEventAuthLoginFailurePayload = {
  ip: string;
  attempts: number;
  retry_after_seconds: number;
  blocked_until?: string;
  method?: AuthMethod;
  credential_name?: string;
  linked_totp_name?: string;
  user_agent?: string;
};

export type SystemEventSessionIpDriftPayload = {
  session_id: string;
  auth_method?: AuthMethod;
  credential_id?: string;
  credential_name?: string;
  linked_totp_name?: string;
  session_comment?: string;
  drift_source: SystemEventSessionDriftSource;
  from_ip: string;
  from_ip_location?: string;
  to_ip: string;
  to_ip_location?: string;
  login_time?: string;
};

export type SystemEventScannerBlockedPayload = {
  ip: string;
  blocked_at: string;
  window_minutes: number;
  threshold: number;
  hit_count: number;
  hits: Array<{
    path: string;
    created_at: string;
  }>;
  ip_location?: string;
};

export type SystemEventDDNSUpdateCompletedPayload = {
  trigger: "cron" | "enable" | "manual_test" | "startup";
  target_id: string;
  target_name: string;
  domain_summary?: string;
  is_primary: boolean;
  provider: string;
  success: boolean;
  message: string;
  update_scope: "dual_stack" | "ipv4_only" | "ipv6_only";
  ip_source: "public" | "interface" | "static" | "domain";
  previous_ipv4?: string | null;
  previous_ipv6?: string | null;
  next_ipv4?: string | null;
  next_ipv6?: string | null;
};

export type SystemEventGatewayThrottleBlockedPayload = {
  ip: string;
  blocked_until: string;
  block_seconds: number;
  requests_per_second: number;
  burst: number;
  route_type?: string;
  host?: string;
  path?: string;
  is_auth_route: boolean;
};

export type SystemEventWAFBlockedPayload = {
  ip: string;
  trace_id: string;
  blocked_at: string;
  mode: string;
  action: string;
  status?: number;
  host?: string;
  path?: string;
  request_uri?: string;
  route_type?: string;
  route_key?: string;
  bundle_id?: string;
  rule_ids: number[];
};

export type SystemEventSSHLoginSuccessPayload = {
  ip: string;
  ip_location?: string;
  username: string;
  auth_method?: string;
  port?: number;
  log_time: string;
};

export type SystemEventSSHLoginFailurePayload = {
  ip: string;
  ip_location?: string;
  username: string;
  invalid_user: boolean;
  auth_method?: string;
  port?: number;
  attempts: number;
  window_minutes: number;
  threshold: number;
  log_time: string;
};

export type SystemEventSSHIPBlockedPayload = {
  ip: string;
  ip_location?: string;
  blocked_at: string;
  blocked_until: string;
  block_seconds: number;
  reason: "failed_login_threshold" | "cidr_not_allowed";
  failed_count: number;
  window_minutes: number;
  threshold: number;
  username?: string;
};

export type SystemEventResourceAlertPayload = {
  hostname: string;
  usage_percent: number;
  threshold_percent: number;
  recover_percent: number;
  sample_interval_seconds: number;
  sustain_seconds: number;
};

export type SystemEventAppUpdateAvailablePayload = {
  local_version: string;
  latest_version: string;
  force_update: boolean;
  release_notes?: string;
  check_reason?: string;
};

export type TunnelType = "frp" | "cloudflared";

export type SystemEventTunnelConnectivityPayload = {
  tunnel: TunnelType;
  status: "connected" | "disconnected";
  pid?: number;
  message?: string;
};

export type SystemEventPayloadMap = {
  [FN_EVENT_AUTH_LOGIN_SUCCESS]: SystemEventAuthLoginSuccessPayload;
  [FN_EVENT_AUTH_LOGOUT]: SystemEventAuthLogoutPayload;
  [FN_EVENT_AUTH_LOGIN_FAILURE]: SystemEventAuthLoginFailurePayload;
  [FN_EVENT_AUTH_SESSION_IP_DRIFT]: SystemEventSessionIpDriftPayload;
  [FN_EVENT_SECURITY_SCANNER_BLOCKED]: SystemEventScannerBlockedPayload;
  [FN_EVENT_DDNS_UPDATE_COMPLETED]: SystemEventDDNSUpdateCompletedPayload;
  [FN_EVENT_GATEWAY_THROTTLE_BLOCKED]: SystemEventGatewayThrottleBlockedPayload;
  [FN_EVENT_WAF_BLOCKED]: SystemEventWAFBlockedPayload;
  [FN_EVENT_SSH_LOGIN_SUCCESS]: SystemEventSSHLoginSuccessPayload;
  [FN_EVENT_SSH_LOGIN_FAILURE]: SystemEventSSHLoginFailurePayload;
  [FN_EVENT_SSH_IP_BLOCKED]: SystemEventSSHIPBlockedPayload;
  [FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE]: SystemEventAppUpdateAvailablePayload;
  [FN_EVENT_SYSTEM_CPU_ALERT]: SystemEventResourceAlertPayload;
  [FN_EVENT_SYSTEM_CPU_RECOVERED]: SystemEventResourceAlertPayload;
  [FN_EVENT_SYSTEM_MEMORY_ALERT]: SystemEventResourceAlertPayload;
  [FN_EVENT_SYSTEM_MEMORY_RECOVERED]: SystemEventResourceAlertPayload;
  [FN_EVENT_TUNNEL_FRP_CONNECTED]: SystemEventTunnelConnectivityPayload;
  [FN_EVENT_TUNNEL_FRP_DISCONNECTED]: SystemEventTunnelConnectivityPayload;
  [FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED]: SystemEventTunnelConnectivityPayload;
  [FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED]: SystemEventTunnelConnectivityPayload;
};

export type SystemEventEnvelope<T extends SystemEventType = SystemEventType> = {
  id: string;
  type: T;
  source: SystemEventSource;
  level: SystemEventLevel;
  happened_at: string;
  dedupe_key?: string;
  subject?: SystemEventSubject;
  tags?: string[];
  payload: SystemEventPayloadMap[T];
};

export type SystemEventPublishInput<
  T extends SystemEventType = SystemEventType,
> = {
  type: T;
  source: SystemEventSource;
  level?: SystemEventLevel;
  happened_at?: string;
  dedupe_key?: string;
  dedupe_ttl_seconds?: number;
  subject?: SystemEventSubject;
  tags?: string[];
  payload: SystemEventPayloadMap[T];
};

export type SystemEventListQuery = {
  page: number;
  limit: number;
  search?: string;
  type?: SystemEventType;
  level?: SystemEventLevel;
  source?: SystemEventSource;
};

export type SystemEventListResult = {
  events: SystemEventEnvelope[];
  total: number;
};

export type SystemEventRangeItem = {
  id: string;
  timestamp: number;
  event: SystemEventEnvelope;
};
